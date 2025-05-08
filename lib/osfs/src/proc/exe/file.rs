use alloc::boxed::Box;
use async_trait::async_trait;
use crate_interface::call_interface;
use systype::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

#[crate_interface::def_interface]
pub trait KernelProcIf {
    fn exe() -> alloc::string::String;
}

pub struct ExeFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for ExeFile {
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

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_readlink(&self, buf: &mut [u8]) -> SysResult<usize> {
        let exe = call_interface!(KernelProcIf::exe());
        log::info!("[/proc/self/exe] run {}", exe);
        if buf.len() < exe.len() {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[..exe.len()].copy_from_slice(exe.as_bytes());
        Ok(exe.len())
    }
}
