use alloc::boxed::Box;

use async_trait::async_trait;
use crate_interface::call_interface;

use crate::proc::{__KernelProcIf_mod, stat::inode::StatInode};
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct StatFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for StatFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SyscallResult {
        let inode = self
            .inode()
            .downcast_arc::<StatInode>()
            .unwrap_or_else(|_| unreachable!());
        let tid = inode.thread_id;
        let stat = if tid == 0 {
            call_interface!(KernelProcIf::stat())
        } else {
            call_interface!(KernelProcIf::stat_from_tid(tid))
        };
        log::info!("[/proc/self/stat] read {}", stat);
        if buf.len() < stat.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..stat.len()].copy_from_slice(stat.as_bytes());
        Ok(stat.len())
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_readlink(&self, buf: &mut [u8]) -> SysResult<usize> {
        let inode = self
            .inode()
            .downcast_arc::<StatInode>()
            .unwrap_or_else(|_| unreachable!());
        let tid = inode.thread_id;
        let stat = if tid == 0 {
            call_interface!(KernelProcIf::stat())
        } else {
            call_interface!(KernelProcIf::stat_from_tid(tid))
        };
        if buf.len() < stat.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..stat.len()].copy_from_slice(stat.as_bytes());
        Ok(stat.len())
    }
}
