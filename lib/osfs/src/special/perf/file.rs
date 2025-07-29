use alloc::boxed::Box;
use alloc::sync::Arc;
use async_trait::async_trait;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::{
    event::PerfEventAttr,
    flags::{PerfEventAttrFlags, PerfType},
    inode::PerfEventInode,
};

pub struct PerfEventFile {
    meta: FileMeta,
}

impl PerfEventFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// Enable the perf event
    pub fn enable(&self) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.enable()
    }

    /// Disable the perf event
    pub fn disable(&self) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.disable()
    }

    /// Reset the perf event counter
    pub fn reset(&self) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.reset()
    }

    /// Get event attributes
    pub fn get_attr(&self) -> SysResult<PerfEventAttr> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        let attr = perf_inode.get_pattr();
        Ok(attr.clone())
    }

    /// Get event ID
    pub fn get_id(&self) -> SysResult<u64> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(perf_inode.get_id())
    }

    /// Check if event is enabled
    pub fn is_enabled(&self) -> SysResult<bool> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(perf_inode.is_enabled())
    }

    /// Increment counter (called by kernel events)
    pub fn increment(&self, delta: u64) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.increment(delta);
        Ok(())
    }

    /// Handle hardware event
    pub fn handle_hardware_event(&self, hw_event: super::flags::PerfHwId) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.handle_hardware_event(hw_event);
        Ok(())
    }

    /// Handle software event
    pub fn handle_software_event(&self, sw_event: super::flags::PerfSwIds) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.handle_software_event(sw_event);
        Ok(())
    }

    /// Check if data is available for reading
    pub fn has_data(&self) -> SysResult<bool> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(perf_inode.has_data())
    }

    /// Get monitored process ID
    pub fn get_pid(&self) -> SysResult<i32> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(perf_inode.get_pid())
    }

    /// Get monitored CPU
    pub fn get_cpu(&self) -> SysResult<i32> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(perf_inode.get_cpu())
    }

    /// Setup the performance event
    pub fn setup(&self) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.setup_hardware_counter()
    }

    /// set group leader
    pub fn set_group_leader(&self, leader: Option<Arc<PerfEventFile>>) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.set_group_leader(leader)
    }

    /// get group leader
    pub fn get_group_leader(&self) -> Option<Arc<PerfEventFile>> {
        let inode = self.inode();
        let perf_inode = inode.downcast_arc::<PerfEventInode>().ok()?;

        perf_inode.get_group_leader()
    }

    /// add group member
    pub fn add_group_member(&self, member: alloc::sync::Weak<PerfEventFile>) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.add_group_member(member)
    }

    /// sync timebase with leader
    pub fn sync_with_leader(&self, leader: &Arc<PerfEventFile>) -> SysResult<()> {
        let inode = self.inode();
        let perf_inode = inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        let leader_inode = leader.inode();
        let leader_perf_inode = leader_inode
            .downcast_arc::<PerfEventInode>()
            .map_err(|_| SysError::EINVAL)?;

        perf_inode.sync_with_leader(&leader_perf_inode)
    }
}

/// Future for asynchronous reading
pub struct PerfEventReadFuture<'a> {
    file: &'a PerfEventFile,
    buf: &'a mut [u8],
    registered: bool,
    read_samples: bool,
}

impl<'a> PerfEventReadFuture<'a> {
    pub fn new(file: &'a PerfEventFile, buf: &'a mut [u8]) -> Self {
        let attr = file.get_attr().unwrap();
        let read_samples = attr.sample_type != 0; // If sample_type is set, read samples

        Self {
            file,
            buf,
            registered: false,
            read_samples,
        }
    }
}

impl<'a> Future for PerfEventReadFuture<'a> {
    type Output = SysResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inode = self.file.inode();
        let perf_inode = match inode.downcast_arc::<PerfEventInode>() {
            Ok(inode) => inode,
            Err(_) => return Poll::Ready(Err(SysError::EINVAL)),
        };

        // Try to read data
        let result = if self.read_samples {
            // Read samples from ring buffer
            perf_inode.read_samples(self.buf)
        } else {
            // Read counter value
            perf_inode.read_count(self.buf)
        };

        match result {
            Ok(bytes) if bytes > 0 => {
                return Poll::Ready(Ok(bytes));
            }
            Ok(_) => {
                // No data available, for counter reads this is OK
                if !self.read_samples {
                    return Poll::Ready(Ok(0));
                }

                // For sample reads, check if we should block
                let attr = perf_inode.get_pattr();
                let flags = PerfEventAttrFlags::from_bits_truncate(attr.flags);
                // In real implementation, would check O_NONBLOCK on file descriptor
                if flags.contains(PerfEventAttrFlags::DISABLED) {
                    return Poll::Ready(Err(SysError::EAGAIN));
                }
            }
            Err(e) => return Poll::Ready(Err(e)),
        }

        // Register waker and wait for more data
        if !self.registered && self.read_samples {
            perf_inode.register_waker(cx.waker().clone());
            self.registered = true;
        }

        if self.read_samples {
            Poll::Pending
        } else {
            Poll::Ready(Ok(0))
        }
    }
}

#[async_trait]
impl File for PerfEventFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        PerfEventReadFuture::new(self, buf).await
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}

// Global registry for managing perf events
use alloc::sync::Weak;
use alloc::vec::Vec;
use spin::Mutex;

static PERF_EVENT_REGISTRY: Mutex<Vec<Weak<PerfEventFile>>> = Mutex::new(Vec::new());

/// Register a perf event globally
pub fn register_perf_event(event: &Arc<PerfEventFile>) {
    let mut registry = PERF_EVENT_REGISTRY.lock();
    registry.push(Arc::downgrade(event));
}

/// Notify all registered perf events of a hardware event
pub fn notify_hardware_event(hw_event: super::flags::PerfHwId, cpu: i32) {
    let mut registry = PERF_EVENT_REGISTRY.lock();
    registry.retain(|weak_ref| {
        if let Some(event) = weak_ref.upgrade() {
            // Check if this event should be notified
            if let Ok(event_cpu) = event.get_cpu() {
                if event_cpu == -1 || event_cpu == cpu {
                    if let Ok(attr) = event.get_attr() {
                        if attr.r#type == PerfType::Hardware as u32
                            && attr.config == hw_event as u64
                        {
                            let _ = event.handle_hardware_event(hw_event);
                        }
                    }
                }
            }
            true
        } else {
            false // Remove dead references
        }
    });
}

/// Notify all registered perf events of a software event
pub fn notify_software_event(sw_event: super::flags::PerfSwIds, pid: i32) {
    let mut registry = PERF_EVENT_REGISTRY.lock();
    registry.retain(|weak_ref| {
        if let Some(event) = weak_ref.upgrade() {
            // Check if this event should be notified
            if let Ok(event_pid) = event.get_pid() {
                if event_pid == -1 || event_pid == pid {
                    if let Ok(attr) = event.get_attr() {
                        if attr.r#type == PerfType::Software as u32
                            && attr.config == sw_event as u64
                        {
                            let _ = event.handle_software_event(sw_event);
                        }
                    }
                }
            }
            true
        } else {
            false // Remove dead references
        }
    });
}
