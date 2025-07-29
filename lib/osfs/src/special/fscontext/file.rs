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
    event::{FsConfigCommand, FsContext},
    flags::FsopenFlags,
    inode::FsContextInode,
};

pub struct FsContextFile {
    meta: FileMeta,
}

impl FsContextFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// Execute a configuration command (used by fsconfig syscall)
    pub fn execute_command(&self, cmd: FsConfigCommand) -> SysResult<()> {
        let inode = self.inode();
        let fs_inode = inode
            .downcast_arc::<FsContextInode>()
            .map_err(|_| SysError::EINVAL)?;

        fs_inode.execute_command(cmd)
    }

    /// Get the current filesystem context (for fsmount syscall)
    pub fn get_context(&self) -> SysResult<FsContext> {
        let inode = self.inode();
        let fs_inode = inode
            .downcast_arc::<FsContextInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(fs_inode.get_context())
    }

    /// Check if ready for mounting
    pub fn is_ready_for_mount(&self) -> SysResult<bool> {
        let inode = self.inode();
        let fs_inode = inode
            .downcast_arc::<FsContextInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(fs_inode.is_ready_for_mount())
    }

    /// Get filesystem type
    pub fn get_fs_type(&self) -> SysResult<alloc::string::String> {
        let inode = self.inode();
        let fs_inode = inode
            .downcast_arc::<FsContextInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(fs_inode.get_fs_type())
    }

    /// Get flags
    pub fn get_flags(&self) -> SysResult<FsopenFlags> {
        let inode = self.inode();
        let fs_inode = inode
            .downcast_arc::<FsContextInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(fs_inode.get_flags())
    }
}

/// Future for asynchronous reading (error log)
pub struct FsContextReadFuture<'a> {
    file: &'a FsContextFile,
    buf: &'a mut [u8],
    registered: bool,
}

impl<'a> FsContextReadFuture<'a> {
    pub fn new(file: &'a FsContextFile, buf: &'a mut [u8]) -> Self {
        Self {
            file,
            buf,
            registered: false,
        }
    }
}

impl<'a> Future for FsContextReadFuture<'a> {
    type Output = SysResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inode = self.file.inode();
        let fs_inode = match inode.downcast_arc::<FsContextInode>() {
            Ok(inode) => inode,
            Err(_) => return Poll::Ready(Err(SysError::EINVAL)),
        };

        // Try to read error log
        match fs_inode.read_error_log(self.buf) {
            Ok(bytes) if bytes > 0 => {
                return Poll::Ready(Ok(bytes));
            }
            Ok(_) => {
                // No data available
                return Poll::Ready(Err(SysError::ENODATA));
            }
            Err(e) => return Poll::Ready(Err(e)),
        }
    }
}

#[async_trait]
impl File for FsContextFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        // Reading from fscontext fd returns error logs
        FsContextReadFuture::new(self, buf).await
    }

    async fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        // Writing to fscontext fd is used for configuration
        // In the real implementation, this would parse configuration strings
        // For now, just return the number of bytes written
        Ok(buf.len())
    }
}
