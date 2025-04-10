use alloc::sync::Arc;
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

impl File for SimpleDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }

    fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        Err(SysError::EISDIR)
    }
}
