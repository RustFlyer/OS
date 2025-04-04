extern crate alloc;
use alloc::sync::Arc;
use config::{
    device::BLOCK_SIZE,
    inode::{InodeMode, InodeType},
    vfs::Stat,
};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    superblock::{self, SuperBlock},
};
pub struct ExtLinkInode {
    meta: InodeMeta,
}

impl ExtLinkInode {
    pub fn new(target: &str, superblock: Arc<dyn SuperBlock>) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(0, superblock),
        })
    }
}

impl Inode for ExtLinkInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: self.meta.inner.lock().mode.bits(),
            st_nlink: 0,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: self.meta.inner.lock().size as u64,
            st_blksize: BLOCK_SIZE as u32,
            __pad2: 0,
            st_blocks: (self.meta.inner.lock().size / BLOCK_SIZE) as u64,
            st_atime: self.meta.inner.lock().atime,
            st_mtime: self.meta.inner.lock().mtime,
            st_ctime: self.meta.inner.lock().ctime,
            unused: 0,
        })
    }
}
