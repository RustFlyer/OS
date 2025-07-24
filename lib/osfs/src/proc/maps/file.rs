use alloc::boxed::Box;

use async_trait::async_trait;
use crate_interface::call_interface;

use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::proc::{__KernelProcIf_mod, maps::inode::MapsInode};

pub struct MapsFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for MapsFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SyscallResult {
        let inode = self
            .inode()
            .downcast_arc::<MapsInode>()
            .unwrap_or_else(|_| unreachable!());
        let tid = inode.thread_id;
        let maps = if tid == 0 {
            call_interface!(KernelProcIf::maps())
        } else {
            call_interface!(KernelProcIf::maps_from_tid(tid))
        };
        if buf.len() < maps.len() {
            log::warn!("buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..maps.len()].copy_from_slice(maps.as_bytes());
        Ok(maps.len())
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        panic!("MapsFile does not support readlink");
    }
}
