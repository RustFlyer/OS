use alloc::boxed::Box;
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct FullFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for FullFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        Ok(0)
    }

    async fn base_write(&self, _buf: &[u8], _pos: usize) -> SysResult<usize> {
        Err(SysError::ENOSPC)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
