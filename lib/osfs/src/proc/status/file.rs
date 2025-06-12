use alloc::boxed::Box;

use async_trait::async_trait;
use crate_interface::call_interface;

use crate::proc::__KernelProcIf_mod;
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct StatusFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for StatusFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SyscallResult {
        let status = call_interface!(KernelProcIf::status());
        log::info!("[/proc/self/status] read {}", status);
        if buf.len() < status.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..status.len()].copy_from_slice(status.as_bytes());
        Ok(status.len())
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
        let status = call_interface!(KernelProcIf::status());
        log::info!("[/proc/self/status] run {}", status);
        if buf.len() < status.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..status.len()].copy_from_slice(status.as_bytes());
        Ok(status.len())
    }
}
