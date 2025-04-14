use alloc::sync::Arc;
use async_trait::async_trait;

use crate::{
    dentry::ExtDentry,
    ext::file::{ExtFile, FileSeekType},
    inode::file::ExtFileInode,
};
use alloc::boxed::Box;
use mutex::ShareMutex;
use systype::{SysError, SysResult};
use vfs::file::{File, FileMeta};

pub struct ExtFileFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileFile {}
unsafe impl Sync for ExtFileFile {}

impl ExtFileFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for ExtFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let mut file = self.file.lock();
        file.seek(pos as i64, FileSeekType::SeekSet)
            .map_err(SysError::from_i32)?;
        let bytes_read = file.read(buf).map_err(SysError::from_i32)?;
        buf[bytes_read..].fill(0);
        Ok(bytes_read)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        let mut file = self.file.lock();
        file.seek(pos as i64, FileSeekType::SeekSet)
            .map_err(SysError::from_i32)?;
        file.write(buf).map_err(SysError::from_i32)
    }
}
