use alloc::sync::Arc;

use config::device::BLOCK_SIZE;
use mutex::{ShareMutex, new_share_mutex};
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    stat::Stat,
    superblock::SuperBlock,
};

use crate::ext::dir::ExtDir;

pub struct ExtDirInode {
    meta: InodeMeta,
    pub(crate) dir: ShareMutex<ExtDir>,
}

unsafe impl Send for ExtDirInode {}
unsafe impl Sync for ExtDirInode {}

impl ExtDirInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, dir: ExtDir) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(dir.as_file().ino() as usize, superblock),
            dir: new_share_mutex(dir),
        })
    }
}

impl Inode for ExtDirInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 2,
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
