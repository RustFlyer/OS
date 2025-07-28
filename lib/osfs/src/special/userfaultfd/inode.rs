use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    event::{UserfaultfdMsg, UserfaultfdRange},
    flags::{UserfaultfdFeatures, UserfaultfdFlags, UserfaultfdIoctls},
};

pub struct UserfaultfdInode {
    meta: InodeMeta,
    flags: UserfaultfdFlags,
    /// API version negotiated with userspace
    api_version: AtomicU64,
    /// Features enabled
    features: AtomicU64,
    /// Available ioctls
    ioctls: AtomicU64,
    /// Whether API handshake completed
    api_initialized: AtomicBool,
    /// Registered memory ranges
    ranges: SpinNoIrqLock<BTreeMap<u64, UserfaultfdRange>>,
    /// Event queue
    events: SpinNoIrqLock<VecDeque<UserfaultfdMsg>>,
    /// Waker queue for blocked readers
    wakers: SpinNoIrqLock<Vec<Waker>>,
    /// Maximum events in queue
    max_events: usize,
    /// Associated process memory management
    mm_id: u64,
}

impl UserfaultfdInode {
    pub fn new(flags: UserfaultfdFlags, mm_id: u64) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            flags,
            api_version: AtomicU64::new(0),
            features: AtomicU64::new(0),
            ioctls: AtomicU64::new(0),
            api_initialized: AtomicBool::new(false),
            ranges: SpinNoIrqLock::new(BTreeMap::new()),
            events: SpinNoIrqLock::new(VecDeque::new()),
            wakers: SpinNoIrqLock::new(Vec::new()),
            max_events: 64, // Reasonable default
            mm_id,
        })
    }

    /// Initialize API version and features
    pub fn initialize_api(
        &self,
        api_version: u64,
        requested_features: u64,
    ) -> SysResult<(u64, u64)> {
        if api_version != super::flags::UFFD_API {
            return Err(SysError::EINVAL);
        }

        // Determine supported features
        let mut supported_features = UserfaultfdFeatures::empty();
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_EVENT_FORK;
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_EVENT_REMAP;
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_EVENT_REMOVE;
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_EVENT_UNMAP;
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_MISSING_SHMEM;
        supported_features |= UserfaultfdFeatures::UFFD_FEATURE_SIGBUS;

        let enabled_features = supported_features.bits() & requested_features;

        // Set available ioctls
        let mut available_ioctls = UserfaultfdIoctls::empty();
        available_ioctls |= UserfaultfdIoctls::UFFDIO_REGISTER;
        available_ioctls |= UserfaultfdIoctls::UFFDIO_UNREGISTER;
        available_ioctls |= UserfaultfdIoctls::UFFDIO_WAKE;
        available_ioctls |= UserfaultfdIoctls::UFFDIO_COPY;
        available_ioctls |= UserfaultfdIoctls::UFFDIO_ZEROPAGE;

        self.api_version.store(api_version, Ordering::Relaxed);
        self.features.store(enabled_features, Ordering::Relaxed);
        self.ioctls
            .store(available_ioctls.bits(), Ordering::Relaxed);
        self.api_initialized.store(true, Ordering::Relaxed);

        Ok((enabled_features, available_ioctls.bits()))
    }

    /// Register a memory range for userfault handling
    pub fn register_range(&self, start: u64, len: u64, mode: u64) -> SysResult<u64> {
        if !self.api_initialized.load(Ordering::Relaxed) {
            return Err(SysError::EINVAL);
        }

        // Validate alignment
        if start % 4096 != 0 || len % 4096 != 0 {
            return Err(SysError::EINVAL);
        }

        if len == 0 {
            return Err(SysError::EINVAL);
        }

        let range = UserfaultfdRange::new(start, len, mode);
        let mut ranges = self.ranges.lock();

        // Check for overlaps
        for existing in ranges.values() {
            if !(range.end <= existing.start || range.start >= existing.end) {
                return Err(SysError::EEXIST);
            }
        }

        ranges.insert(start, range);
        Ok(UserfaultfdIoctls::all().bits()) // Return available ioctls for this range
    }

    /// Unregister a memory range
    pub fn unregister_range(&self, start: u64, len: u64) -> SysResult<()> {
        if !self.api_initialized.load(Ordering::Relaxed) {
            return Err(SysError::EINVAL);
        }

        let mut ranges = self.ranges.lock();
        let end = start + len;

        // Remove ranges that overlap with the specified region
        ranges.retain(|_, range| !(range.start < end && range.end > start));

        Ok(())
    }

    /// Handle a page fault
    pub fn handle_pagefault(&self, address: u64, flags: u64, ptid: u32) -> SysResult<()> {
        let ranges = self.ranges.lock();

        // Check if address is in a registered range
        let mut in_range = false;
        for range in ranges.values() {
            if range.contains(address) {
                in_range = true;
                break;
            }
        }
        drop(ranges);

        if !in_range {
            return Err(SysError::EFAULT);
        }

        // Create and queue pagefault event
        let msg = UserfaultfdMsg::new_pagefault(address, flags, ptid);
        self.push_event(msg);

        Ok(())
    }

    /// Push an event to the queue
    pub fn push_event(&self, event: UserfaultfdMsg) {
        let mut events = self.events.lock();

        if events.len() >= self.max_events {
            // Drop oldest event
            events.pop_front();
        }

        events.push_back(event);
        drop(events);

        // Wake up waiting readers
        self.wake_all_readers();
    }

    /// Wake all waiting readers
    fn wake_all_readers(&self) {
        let mut wakers = self.wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    /// Register a waker for event notifications
    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.wakers.lock();

        // Check if we already have events
        if self.has_events_internal() {
            waker.wake();
        } else {
            // Avoid duplicates
            if !wakers.iter().any(|w| w.will_wake(&waker)) {
                wakers.push(waker);
            }
        }
    }

    /// Read events from the queue
    pub fn read_events(&self, buf: &mut [u8]) -> SysResult<usize> {
        if !self.api_initialized.load(Ordering::Relaxed) {
            return Err(SysError::EINVAL);
        }

        let mut events = self.events.lock();
        let mut total_bytes = 0;
        let mut buf_offset = 0;

        while let Some(_event) = events.front() {
            let event_size = UserfaultfdMsg::serialized_size();
            if buf_offset + event_size > buf.len() {
                break;
            }

            let event = events.pop_front().unwrap();
            match event.serialize_into(&mut buf[buf_offset..]) {
                Ok(bytes) => {
                    buf_offset += bytes;
                    total_bytes += bytes;
                }
                Err(_) => break,
            }
        }

        if total_bytes == 0 && self.flags.contains(UserfaultfdFlags::UFFD_NONBLOCK) {
            return Err(SysError::EAGAIN);
        }

        Ok(total_bytes)
    }

    /// Check if there are events available
    pub fn has_events(&self) -> bool {
        self.has_events_internal()
    }

    fn has_events_internal(&self) -> bool {
        !self.events.lock().is_empty()
    }

    /// Get current flags
    pub fn get_flags(&self) -> UserfaultfdFlags {
        self.flags
    }

    /// Get API info
    pub fn get_api_info(&self) -> (u64, u64, u64) {
        (
            self.api_version.load(Ordering::Relaxed),
            self.features.load(Ordering::Relaxed),
            self.ioctls.load(Ordering::Relaxed),
        )
    }

    /// Copy data to resolve a pagefault
    pub fn copy_page(&self, dst: u64, src: &[u8], mode: u64) -> SysResult<usize> {
        if src.len() != 4096 {
            return Err(SysError::EINVAL);
        }

        // In a real implementation, this would:
        // 1. Validate the destination address is in a registered range
        // 2. Copy the data to the process's memory space
        // 3. Update page tables
        // 4. Wake up any threads waiting on this address

        todo!()
    }

    /// Create zero page to resolve a pagefault
    pub fn zeropage(&self, dst: u64, len: u64, mode: u64) -> SysResult<usize> {
        if len % 4096 != 0 || dst % 4096 != 0 {
            return Err(SysError::EINVAL);
        }

        // In real implementation, would zero-fill pages and update mappings
        todo!()
    }

    /// Wake up threads waiting on a range
    pub fn wake_range(&self, start: u64, len: u64) -> SysResult<usize> {
        // In real implementation, would wake threads blocked on addresses in range
        // Return number of woken threads
        todo!()
    }
}

impl Inode for UserfaultfdInode {
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
