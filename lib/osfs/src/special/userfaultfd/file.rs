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

use super::{flags::UserfaultfdFlags, inode::UserfaultfdInode};

pub struct UserfaultfdFile {
    meta: FileMeta,
}

impl UserfaultfdFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// Initialize API
    pub fn initialize_api(&self, api_version: u64, features: u64) -> SysResult<(u64, u64)> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.initialize_api(api_version, features)
    }

    /// Register memory range
    pub fn register_range(&self, start: u64, len: u64, mode: u64) -> SysResult<u64> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.register_range(start, len, mode)
    }

    /// Unregister memory range
    pub fn unregister_range(&self, start: u64, len: u64) -> SysResult<()> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.unregister_range(start, len)
    }

    /// Handle pagefault
    pub fn handle_pagefault(&self, address: u64, flags: u64, ptid: u32) -> SysResult<()> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.handle_pagefault(address, flags, ptid)
    }

    /// Copy page to resolve fault
    pub fn copy_page(&self, dst: u64, src: &[u8], mode: u64) -> SysResult<usize> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.copy_page(dst, src, mode)
    }

    /// Create zero page
    pub fn zeropage(&self, dst: u64, len: u64, mode: u64) -> SysResult<usize> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.zeropage(dst, len, mode)
    }

    /// Wake threads waiting on range
    pub fn wake_range(&self, start: u64, len: u64) -> SysResult<usize> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        uffd_inode.wake_range(start, len)
    }

    /// Get API info
    pub fn get_api_info(&self) -> SysResult<(u64, u64, u64)> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(uffd_inode.get_api_info())
    }

    /// Check if events are available
    pub fn has_events(&self) -> SysResult<bool> {
        let inode = self.inode();
        let uffd_inode = inode
            .downcast_arc::<UserfaultfdInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(uffd_inode.has_events())
    }
}

/// Future for asynchronous reading
pub struct UserfaultfdReadFuture<'a> {
    file: &'a UserfaultfdFile,
    buf: &'a mut [u8],
    registered: bool,
}

impl<'a> UserfaultfdReadFuture<'a> {
    pub fn new(file: &'a UserfaultfdFile, buf: &'a mut [u8]) -> Self {
        Self {
            file,
            buf,
            registered: false,
        }
    }
}

impl<'a> Future for UserfaultfdReadFuture<'a> {
    type Output = SysResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inode = self.file.inode();
        let uffd_inode = match inode.downcast_arc::<UserfaultfdInode>() {
            Ok(inode) => inode,
            Err(_) => return Poll::Ready(Err(SysError::EINVAL)),
        };

        // Try to read events
        match uffd_inode.read_events(self.buf) {
            Ok(bytes) if bytes > 0 => {
                return Poll::Ready(Ok(bytes));
            }
            Ok(_) => {
                // No events, check if non-blocking
                if uffd_inode
                    .get_flags()
                    .contains(UserfaultfdFlags::UFFD_NONBLOCK)
                {
                    return Poll::Ready(Err(SysError::EAGAIN));
                }
            }
            Err(e) => return Poll::Ready(Err(e)),
        }

        // Register waker and wait
        if !self.registered {
            uffd_inode.register_waker(cx.waker().clone());
            self.registered = true;
        }

        Poll::Pending
    }
}

#[async_trait]
impl File for UserfaultfdFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        UserfaultfdReadFuture::new(self, buf).await
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}
