use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;

use systype::error::{SysError, SysResult, SyscallResult};

use crate::file::{File, FileMeta};

use super::inode::FanotifyEventInode;

/// File implementation for fanotify event file descriptor.
///
/// This file represents the special file descriptor created for each fanotify event.
pub struct FanotifyEventFile {
    pub(crate) meta: FileMeta,
}

impl FanotifyEventFile {
    /// Gets the associated fanotify event inode
    fn event_inode(&self) -> Arc<FanotifyEventInode> {
        self.inode()
            .downcast_arc::<FanotifyEventInode>()
            .unwrap_or_else(|_| panic!("Expected FanotifyEventInode"))
    }
}

#[async_trait]
impl File for FanotifyEventFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _offset: usize) -> SyscallResult {
        Err(SysError::EBADF)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EBADF)
    }

    fn base_read_dir(&self) -> SysResult<Option<crate::direntry::DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}
