use alloc::boxed::Box;
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct ZeroFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for ZeroFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    async fn base_write(&self, buf: &[u8], _pos: usize) -> SysResult<usize> {
        Ok(buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
