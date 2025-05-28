use alloc::sync::Arc;

use config::device::BLOCK_SIZE;
use mutex::{ShareMutex, new_share_mutex};
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    stat::Stat,
    superblock::SuperBlock,
};

use crate::ext::file::ExtFile;

pub struct ExtFileInode {
    meta: InodeMeta,
    pub(crate) file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileInode {}
unsafe impl Sync for ExtFileInode {}

impl ExtFileInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, file: ExtFile) -> Arc<Self> {
        let fsize = file.size();
        let meta = InodeMeta::new(file.ino() as usize, superblock);
        meta.inner.lock().size = fsize as usize;
        Arc::new(Self {
            meta,
            file: new_share_mutex(file),
        })
    }
}

impl Inode for ExtFileInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: inner.size as u64,
            st_blksize: BLOCK_SIZE as u32,
            __pad2: 0,
            st_blocks: (inner.size / BLOCK_SIZE) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
