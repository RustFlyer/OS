use alloc::boxed::Box;
use core::cmp;

use async_trait::async_trait;

use systype::error::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use super::serialize_interrupts;

pub struct InterruptsFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for InterruptsFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let info = serialize_interrupts();
        let len = cmp::min(info.len().saturating_sub(pos), buf.len());
        buf[..len].copy_from_slice(&info.as_bytes()[pos..pos + len]);
        Ok(len)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }
}
