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
            meta: FileMeta::new(dentry.clone()),
            dir: inode.dir.clone(),
        })
    }
}

impl File for ExtDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_load_dir(&self) -> systype::SysResult<()> {
        let mut dir = self.dir.lock();

        dir.rewind();
        dir.next();
        dir.next();

        while let Some(dentry) = dir.next() {
            self.dentry().lookup(&dentry.name()?)?;
        }

        Ok(())
    }
}
