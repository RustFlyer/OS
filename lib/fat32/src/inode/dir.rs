use alloc::sync::Arc;
use config::inode::InodeType;
use mutex::{ShareMutex, new_share_mutex};
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

use crate::FatDir;

pub struct FatDirInode {
    meta: InodeMeta,
    pub dir: ShareMutex<FatDir>,
}

impl FatDirInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, dir: FatDir) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), super_block.clone()),
            dir: new_share_mutex(dir),
        });
        inode.set_size(0);
        inode.set_inotype(InodeType::Dir);
        inode
    }
}

impl Inode for FatDirInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (len / 512) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
