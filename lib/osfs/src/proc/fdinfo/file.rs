use alloc::{boxed::Box, format};

use async_trait::async_trait;
use crate_interface::call_interface;

use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::proc::{__KernelProcIf_mod, fdinfo::inode::FdInfoInode};

pub struct FdInfoFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for FdInfoFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], offset: usize) -> SyscallResult {
        if offset != 0 {
            return Ok(0);
        }

        let inode = self
            .inode()
            .downcast_arc::<FdInfoInode>()
            .unwrap_or_else(|_| unreachable!());
        let tid = inode.thread_id;
        let fd = inode.file_descriptor;

        let fdinfo = call_interface!(KernelProcIf::fdinfo_from_tid_and_fd(tid, fd))?;
        let fdinfo_str = fdinfo.as_text();
        let filelen = fdinfo_str.len();
        if buf.len() < filelen {
            log::warn!("buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..filelen].copy_from_slice(&fdinfo_str.as_bytes());
        Ok(filelen)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        panic!("FdInfoFile does not support readlink");
    }
}
