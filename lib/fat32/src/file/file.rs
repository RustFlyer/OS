use alloc::sync::Arc;
use mutex::ShareMutex;
use systype::SysResult;
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::{FatFile, dentry::FatDentry, inode::file::FatFileInode};

pub struct FatFileFile {
    meta: FileMeta,
    file: ShareMutex<FatFile>,
}

impl FatFileFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        todo!()
    }

    fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        todo!()
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }
}
