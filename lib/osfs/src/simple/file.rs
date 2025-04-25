use alloc::boxed::Box;
use alloc::sync::Arc;
use async_trait::async_trait;
use systype::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};
pub struct SimpleDirFile {
    meta: FileMeta,
}

impl SimpleDirFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

#[async_trait]
impl File for SimpleDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }
}

pub struct SimpleFileFile {
    meta: FileMeta,
}

impl SimpleFileFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

#[async_trait]
impl File for SimpleFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }
}
