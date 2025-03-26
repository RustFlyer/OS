extern crate alloc;

use alloc::sync::Arc;

use mutex::ShareMutex;
use vfs::file::{File, FileMeta};

use crate::{dentry::ExtDentry, ext::dir::ExtDir, inode::dir::ExtDirInode};

pub struct ExtDirFile {
    meta: FileMeta,
    dir: ShareMutex<ExtDir>,
}

unsafe impl Send for ExtDirFile {}
unsafe impl Sync for ExtDirFile {}

impl ExtDirFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            dir: inode.dir.clone(),
        })
    }
}

#[async_trait]
impl File for ExtDirFile {
    fn get_meta(&self) -> &FileMeta {
        &self.meta
    }

    /// # Here We should implement a function to load all dentry and inodes in a directory.
    fn base_load_dir(&self) -> systype::SysResult<()> {
        todo!()
    }
}
