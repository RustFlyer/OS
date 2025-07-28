use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use core::task::Waker;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::{
    event::{
        IoRequest, IoUringCqe, IoUringParams, IoUringRing, IoUringSqe, RegisteredBuffer,
        RegisteredFile,
    },
    flags::IoUringSetupFlags,
};

pub struct IoUringInode {
    meta: InodeMeta,
    /// Setup parameters
    params: SpinNoIrqLock<IoUringParams>,
    /// Setup flags
    flags: IoUringSetupFlags,
    /// Submission queue
    sq_ring: SpinNoIrqLock<IoUringRing<IoUringSqe>>,
    /// Completion queue
    cq_ring: SpinNoIrqLock<IoUringRing<IoUringCqe>>,
    /// Pending requests
    pending_requests: SpinNoIrqLock<BTreeMap<u64, IoRequest>>,
    /// Next request ID
    next_request_id: AtomicU32,
    /// Registered buffers
    registered_buffers: SpinNoIrqLock<BTreeMap<u16, RegisteredBuffer>>,
    /// Registered files
    registered_files: SpinNoIrqLock<BTreeMap<u32, RegisteredFile>>,
    /// Wakers for threads waiting on CQ events
    cq_wakers: SpinNoIrqLock<Vec<Waker>>,
    /// Whether the ring is enabled
    enabled: AtomicBool,
    /// Process ID that owns this ring
    owner_pid: u32,
    /// Features enabled for this ring
    features: u32,
}

impl IoUringInode {
    pub fn new(entries: u32, flags: IoUringSetupFlags, owner_pid: u32) -> SysResult<Arc<Self>> {
        let sq_entries = entries;
        let cq_entries = if flags.contains(IoUringSetupFlags::IORING_SETUP_CQSIZE) {
            entries * 2 // Default CQ size is 2x SQ size
        } else {
            entries
        };

        let mut params = IoUringParams::default();
        params.sq_entries = sq_entries;
        params.cq_entries = cq_entries;
        params.flags = flags.bits();

        // Set up ring offsets (simplified)
        params.sq_off.head = 0;
        params.sq_off.tail = 4;
        params.sq_off.ring_mask = 8;
        params.sq_off.ring_entries = 12;
        params.sq_off.flags = 16;
        params.sq_off.dropped = 20;
        params.sq_off.array = 24;

        params.cq_off.head = 0;
        params.cq_off.tail = 4;
        params.cq_off.ring_mask = 8;
        params.cq_off.ring_entries = 12;
        params.cq_off.overflow = 16;
        params.cq_off.cqes = 20;
        params.cq_off.flags = 24;

        // Set supported features
        params.features = 0; // Add feature flags as needed

        Ok(Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            params: SpinNoIrqLock::new(params),
            flags,
            sq_ring: SpinNoIrqLock::new(IoUringRing::new(sq_entries)),
            cq_ring: SpinNoIrqLock::new(IoUringRing::new(cq_entries)),
            pending_requests: SpinNoIrqLock::new(BTreeMap::new()),
            next_request_id: AtomicU32::new(1),
            registered_buffers: SpinNoIrqLock::new(BTreeMap::new()),
            registered_files: SpinNoIrqLock::new(BTreeMap::new()),
            cq_wakers: SpinNoIrqLock::new(Vec::new()),
            enabled: AtomicBool::new(!flags.contains(IoUringSetupFlags::IORING_SETUP_R_DISABLED)),
            owner_pid,
            features: params.features,
        }))
    }

    pub fn get_params(&self) -> IoUringParams {
        *self.params.lock()
    }

    pub fn submit_sqe(&self, sqe: IoUringSqe) -> SysResult<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(SysError::EBADF);
        }

        let mut sq_ring = self.sq_ring.lock();
        sq_ring.push(sqe).map_err(|_| SysError::EAGAIN)?;

        // Create a pending request
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed) as u64;
        let request = IoRequest::new(sqe);
        self.pending_requests.lock().insert(request_id, request);

        // In a real implementation, this would trigger async processing
        self.process_sq_entries();

        Ok(())
    }

    pub fn get_completion(&self) -> Option<IoUringCqe> {
        let mut cq_ring = self.cq_ring.lock();
        cq_ring.pop()
    }

    pub fn has_completions(&self) -> bool {
        !self.cq_ring.lock().is_empty()
    }

    pub fn wait_for_completions(&self, min_complete: u32) -> SysResult<u32> {
        let available = self.cq_ring.lock().pending_entries();
        if available >= min_complete {
            return Ok(available);
        }

        if self.flags.contains(IoUringSetupFlags::IORING_SETUP_IOPOLL) {
            // In IOPOLL mode, we actively poll for completions
            self.poll_for_completions();
        }

        Ok(self.cq_ring.lock().pending_entries())
    }

    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.cq_wakers.lock();
        if self.has_completions() {
            waker.wake();
        } else if !wakers.iter().any(|w| w.will_wake(&waker)) {
            wakers.push(waker);
        }
    }

    pub fn register_buffers(&self, buffers: &[(u64, u32)]) -> SysResult<()> {
        let mut registered_buffers = self.registered_buffers.lock();

        for (i, &(addr, len)) in buffers.iter().enumerate() {
            if addr == 0 || len == 0 {
                return Err(SysError::EINVAL);
            }

            let buffer = RegisteredBuffer {
                addr,
                len,
                buffer_id: i as u16,
            };

            registered_buffers.insert(i as u16, buffer);
        }

        Ok(())
    }

    pub fn unregister_buffers(&self) -> SysResult<()> {
        self.registered_buffers.lock().clear();
        Ok(())
    }

    pub fn register_files(&self, fds: &[i32]) -> SysResult<()> {
        let mut registered_files = self.registered_files.lock();

        for (i, &fd) in fds.iter().enumerate() {
            if fd < 0 {
                continue; // Allow sparse arrays
            }

            let file = RegisteredFile {
                fd,
                file_index: i as u32,
            };

            registered_files.insert(i as u32, file);
        }

        Ok(())
    }

    pub fn unregister_files(&self) -> SysResult<()> {
        self.registered_files.lock().clear();
        Ok(())
    }

    pub fn enable_rings(&self) -> SysResult<()> {
        self.enabled.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn disable_rings(&self) -> SysResult<()> {
        self.enabled.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn process_sq_entries(&self) {
        // This is a simplified version. In a real implementation, this would:
        // 1. Process each SQE in the submission queue
        // 2. Execute the requested I/O operations asynchronously
        // 3. Generate CQEs when operations complete

        let pending = self.pending_requests.lock().clone();
        for (request_id, mut request) in pending {
            // Simulate processing different operation types
            let result = match request.sqe.opcode {
                op if op == super::flags::IoUringOpcode::IORING_OP_NOP.bits() => 0,
                op if op == super::flags::IoUringOpcode::IORING_OP_READ.bits() => {
                    // In real implementation: perform actual read
                    request.sqe.len as i32
                }
                op if op == super::flags::IoUringOpcode::IORING_OP_WRITE.bits() => {
                    // In real implementation: perform actual write
                    request.sqe.len as i32
                }
                _ => -SysError::ENOSYS.code(), // Unsupported operation
            };

            // Complete the request
            request.complete(result);

            // Add to completion queue
            let cqe = IoUringCqe::new(request.sqe.user_data, result, 0);
            let _ = self.cq_ring.lock().push(cqe);

            // Remove from pending
            self.pending_requests.lock().remove(&request_id);
        }

        // Wake up waiters
        self.wake_all_waiters();
    }

    fn poll_for_completions(&self) {
        // In IOPOLL mode, actively check for I/O completions
        // This would integrate with the actual I/O subsystem
        self.process_sq_entries();
    }

    fn wake_all_waiters(&self) {
        let mut wakers = self.cq_wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    pub fn get_setup_flags(&self) -> IoUringSetupFlags {
        self.flags
    }

    pub fn get_features(&self) -> u32 {
        self.features
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn get_owner_pid(&self) -> u32 {
        self.owner_pid
    }
}

impl Inode for IoUringInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}
