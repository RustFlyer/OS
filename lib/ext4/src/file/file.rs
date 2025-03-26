extern crate alloc;
use alloc::sync::Arc;

use mutex::ShareMutex;
use systype::SyscallResult;
use vfs::{
    dentry,
    file::{File, FileMeta},
};

use crate::{dentry::ExtDentry, ext::file::ExtFile, inode::file::ExtFileInode};

pub struct ExtFileFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileFile {}
unsafe impl Sync for ExtFileFile {}

impl ExtFileFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for ExtFileFile {
    fn get_meta(&self) -> &FileMeta {
        &self.meta
    }

    /// # todo: read datas from file at offset to buf!
    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    /// # todo: write datas from buf to file at offset!
    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        todo!()
    }
}
