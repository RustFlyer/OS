use core::cmp;

use alloc::boxed::Box;
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use super::MEM_INFO;

pub struct MemInfoFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for MemInfoFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let meminfo = MEM_INFO.lock();
        let info = meminfo.serialize();
        let len = cmp::min(info.len() - pos, buf.len());
        buf[..len].copy_from_slice(&info.as_bytes()[pos..pos + len]);
        Ok(len)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
