use alloc::sync::Arc;
use config::{
    device::BLOCK_SIZE,
    inode::{InodeMode, InodeType},
};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

pub struct ZeroInode {
    meta: InodeMeta,
}

impl ZeroInode {
    pub fn new(superblock: Arc<dyn SuperBlock>) -> Arc<Self> {
        let size = BLOCK_SIZE;
        let mode = InodeMode::CHAR;
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), superblock),
        });
        inode.set_inotype(InodeType::from(mode));
        inode.set_size(size);
        inode
    }
}

impl Inode for ZeroInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
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
