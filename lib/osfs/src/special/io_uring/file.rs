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
    event::{IoUringCqe, IoUringParams, IoUringSqe},
    flags::{IoUringEnterFlags, IoUringRegisterOp, IoUringSetupFlags},
    inode::IoUringInode,
};

pub struct IoUringFile {
    meta: FileMeta,
}

impl IoUringFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// Get ring parameters
    pub fn get_params(&self) -> SysResult<IoUringParams> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(io_uring_inode.get_params())
    }

    /// Submit entries and/or wait for completions
    pub fn enter(
        &self,
        to_submit: u32,
        min_complete: u32,
        flags: IoUringEnterFlags,
        sig: Option<u64>, // sigset_t pointer
    ) -> SysResult<u32> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        let mut submitted = 0u32;
        let mut completed = 0u32;

        // Submit entries if requested
        if to_submit > 0 {
            // In a real implementation, we would read SQEs from user memory
            // For now, simulate submission
            submitted = to_submit; // Simplified
        }

        // Wait for completions if requested
        if flags.contains(IoUringEnterFlags::IORING_ENTER_GETEVENTS) {
            completed = io_uring_inode.wait_for_completions(min_complete)?;
        }

        // Wake up SQ thread if needed
        if flags.contains(IoUringEnterFlags::IORING_ENTER_SQ_WAKEUP) {
            // In real implementation: wake up kernel SQ thread
        }

        Ok(
            if flags.contains(IoUringEnterFlags::IORING_ENTER_GETEVENTS) {
                completed
            } else {
                submitted
            },
        )
    }

    /// Register resources with the ring
    pub fn register(&self, opcode: IoUringRegisterOp, arg: u64, nr_args: u32) -> SysResult<u32> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        match opcode {
            IoUringRegisterOp::IORING_REGISTER_BUFFERS => {
                // In real implementation: read buffer array from user memory
                // For now, return success
                io_uring_inode.register_buffers(&[])?;
                Ok(0)
            }
            IoUringRegisterOp::IORING_UNREGISTER_BUFFERS => {
                io_uring_inode.unregister_buffers()?;
                Ok(0)
            }
            IoUringRegisterOp::IORING_REGISTER_FILES => {
                // In real implementation: read fd array from user memory
                io_uring_inode.register_files(&[])?;
                Ok(0)
            }
            IoUringRegisterOp::IORING_UNREGISTER_FILES => {
                io_uring_inode.unregister_files()?;
                Ok(0)
            }
            IoUringRegisterOp::IORING_REGISTER_ENABLE_RINGS => {
                io_uring_inode.enable_rings()?;
                Ok(0)
            }
            _ => {
                log::warn!("[io_uring] Unsupported register operation: {:?}", opcode);
                Err(SysError::EINVAL)
            }
        }
    }

    /// Submit a single SQE (for testing/simple cases)
    pub fn submit_sqe(&self, sqe: IoUringSqe) -> SysResult<()> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        io_uring_inode.submit_sqe(sqe)
    }

    /// Get a completion event
    pub fn get_completion(&self) -> SysResult<Option<IoUringCqe>> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(io_uring_inode.get_completion())
    }

    /// Check if completions are available
    pub fn has_completions(&self) -> SysResult<bool> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(io_uring_inode.has_completions())
    }

    /// Get ring info
    pub fn get_ring_info(&self) -> SysResult<(IoUringSetupFlags, u32, bool)> {
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok((
            io_uring_inode.get_setup_flags(),
            io_uring_inode.get_features(),
            io_uring_inode.is_enabled(),
        ))
    }
}

/// Future for waiting on completions
pub struct IoUringCompletionFuture<'a> {
    file: &'a IoUringFile,
    min_complete: u32,
    registered: bool,
}

impl<'a> IoUringCompletionFuture<'a> {
    pub fn new(file: &'a IoUringFile, min_complete: u32) -> Self {
        Self {
            file,
            min_complete,
            registered: false,
        }
    }
}

impl<'a> Future for IoUringCompletionFuture<'a> {
    type Output = SysResult<u32>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inode = self.file.inode();
        let io_uring_inode = match inode.downcast_arc::<IoUringInode>() {
            Ok(inode) => inode,
            Err(_) => return Poll::Ready(Err(SysError::EINVAL)),
        };

        // Check if we have enough completions
        let available = io_uring_inode.wait_for_completions(self.min_complete);
        match available {
            Ok(count) if count >= self.min_complete => {
                return Poll::Ready(Ok(count));
            }
            Ok(_) => {
                // Not enough completions, continue waiting
            }
            Err(e) => return Poll::Ready(Err(e)),
        }

        // Register waker for completion notifications
        if !self.registered {
            io_uring_inode.register_waker(cx.waker().clone());
            self.registered = true;
        }

        Poll::Pending
    }
}

#[async_trait]
impl File for IoUringFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        // Reading from io_uring fd returns completion events
        let inode = self.inode();
        let io_uring_inode = inode
            .downcast_arc::<IoUringInode>()
            .map_err(|_| SysError::EINVAL)?;

        if let Some(cqe) = io_uring_inode.get_completion() {
            let cqe_size = core::mem::size_of::<IoUringCqe>();
            if buf.len() < cqe_size {
                return Err(SysError::EINVAL);
            }

            unsafe {
                core::ptr::copy_nonoverlapping(
                    &cqe as *const _ as *const u8,
                    buf.as_mut_ptr(),
                    cqe_size,
                );
            }

            Ok(cqe_size)
        } else {
            // No completions available
            if io_uring_inode
                .get_setup_flags()
                .contains(IoUringSetupFlags::IORING_SETUP_IOPOLL)
            {
                return Err(SysError::EAGAIN);
            }

            // Wait for completion
            IoUringCompletionFuture::new(self, 1).await?;
            self.base_read(buf, _pos).await
        }
    }

    async fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        // Writing to io_uring fd submits SQEs
        let sqe_size = core::mem::size_of::<IoUringSqe>();
        if buf.len() < sqe_size {
            return Err(SysError::EINVAL);
        }

        let sqe = unsafe { core::ptr::read(buf.as_ptr() as *const IoUringSqe) };

        self.submit_sqe(sqe)?;
        Ok(sqe_size)
    }
}
