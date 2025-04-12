use alloc::sync::Arc;
use mutex::{ShareMutex, new_share_mutex};
use systype::SysResult;
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::{FatDir, FatDirIter, dentry::FatDentry, inode::dir::FatDirInode};

pub struct FatDirFile {
    meta: FileMeta,
    dir: ShareMutex<FatDir>,
    iter_cache: ShareMutex<FatDirIter>,
}

impl FatDirFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            dir: inode.dir.clone(),
            iter_cache: new_share_mutex(inode.dir.lock().iter()),
        })
    }
}

impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    fn base_load_dir(&self) -> SysResult<()> {
        todo!()
    }
}
