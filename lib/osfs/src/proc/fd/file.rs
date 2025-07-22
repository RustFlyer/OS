use alloc::boxed::Box;

use async_trait::async_trait;
use crate_interface::call_interface;

use crate::proc::{__KernelProcIf_mod, fd::inode::FdInode};
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};
pub struct FdFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for FdFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _offset: usize) -> SyscallResult {
        todo!()
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_readlink(&self, buf: &mut [u8]) -> SysResult<usize> {
        let inode = self
            .inode()
            .downcast_arc::<FdInode>()
            .unwrap_or_else(|_| unreachable!());
        let fd = inode.fd;

        let path = call_interface!(KernelProcIf::fd(fd));
        log::info!("[/proc/self/fd/{}] link {}", fd, path);
        if buf.len() < path.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..path.len()].copy_from_slice(path.as_bytes());
        Ok(path.len())
    }
}
