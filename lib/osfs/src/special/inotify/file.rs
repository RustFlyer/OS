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
    flags::{InotifyFlags, InotifyMask},
    inode::InotifyInode,
};

pub struct InotifyFile {
    meta: FileMeta,
}

impl InotifyFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    pub fn add_watch(
        &self,
        inode_id: u64,
        mask: u32,
        path: Option<alloc::string::String>,
    ) -> SysResult<i32> {
        let inode = self.inode();
        let inotify_inode = inode
            .downcast_arc::<InotifyInode>()
            .map_err(|_| SysError::EINVAL)?;

        // Validate mask
        let valid_mask = InotifyMask::from_bits(mask).ok_or(SysError::EINVAL)?;

        inotify_inode.add_watch(inode_id, valid_mask.bits(), path)
    }

    pub fn remove_watch(&self, wd: i32) -> SysResult<()> {
        let inode = self.inode();
        let inotify_inode = inode
            .downcast_arc::<InotifyInode>()
            .map_err(|_| SysError::EINVAL)?;

        inotify_inode.remove_watch(wd)
    }

    pub fn get_flags(&self) -> SysResult<InotifyFlags> {
        let inode = self.inode();
        let inotify_inode = inode
            .downcast_arc::<InotifyInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(inotify_inode.get_flags())
    }

    pub fn has_events(&self) -> SysResult<bool> {
        let inode = self.inode();
        let inotify_inode = inode
            .downcast_arc::<InotifyInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(inotify_inode.has_events())
    }

    pub fn notify_event(
        &self,
        inode_id: u64,
        mask: u32,
        name: Option<alloc::string::String>,
    ) -> SysResult<()> {
        let inode = self.inode();
        let inotify_inode = inode
            .downcast_arc::<InotifyInode>()
            .map_err(|_| SysError::EINVAL)?;

        inotify_inode.notify_inode_event(inode_id, mask, name);
        Ok(())
    }
}

pub struct InotifyReadFuture<'a> {
    file: &'a InotifyFile,
    buf: &'a mut [u8],
    registered: bool,
}

impl<'a> InotifyReadFuture<'a> {
    pub fn new(file: &'a InotifyFile, buf: &'a mut [u8]) -> Self {
        Self {
            file,
            buf,
            registered: false,
        }
    }
}

impl<'a> Future for InotifyReadFuture<'a> {
    type Output = SysResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inode = self.file.inode();
        let inotify_inode = match inode.downcast_arc::<InotifyInode>() {
            Ok(inode) => inode,
            Err(_) => return Poll::Ready(Err(SysError::EINVAL)),
        };

        match inotify_inode.read_events(self.buf) {
            Ok(bytes) if bytes > 0 => {
                return Poll::Ready(Ok(bytes));
            }
            Ok(_) => {
                if inotify_inode
                    .get_flags()
                    .contains(InotifyFlags::IN_NONBLOCK)
                {
                    return Poll::Ready(Err(SysError::EAGAIN));
                }
            }
            Err(e) => return Poll::Ready(Err(e)),
        }

        if !self.registered {
            inotify_inode.register_waker(cx.waker().clone());
            self.registered = true;
        }

        Poll::Pending
    }
}

#[async_trait]
impl File for InotifyFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        InotifyReadFuture::new(self, buf).await
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}
