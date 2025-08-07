use alloc::sync::Arc;

use config::device::BLOCK_SIZE;
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    stat::Stat,
    superblock::SuperBlock,
};

pub struct ExtLinkInode {
    meta: InodeMeta,
}

impl ExtLinkInode {
    pub fn new(superblock: Arc<dyn SuperBlock>) -> Arc<Self> {
        Arc::new(Self {
            // Lwext4 does not provide a way to get metadata of a symlink.
            // We set the inode number to 100 for now.
            meta: InodeMeta::new(999, superblock),
        })
    }
}

impl Inode for ExtLinkInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: self.superblock().dev_id() << 8,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: inner.nlink as u32,
            st_uid: inner.uid,
            st_gid: inner.gid,
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
