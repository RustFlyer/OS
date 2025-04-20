use alloc::sync::Arc;

use config::{device::BLOCK_SIZE, vfs::Stat};
use mutex::{ShareMutex, new_share_mutex};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    superblock::SuperBlock,
};

use crate::ext::file::ExtFile;
pub struct ExtLinkInode {
    meta: InodeMeta,
    pub(crate) file: ShareMutex<ExtFile>,
}

impl ExtLinkInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, file: ExtFile) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(0, superblock),
            file: new_share_mutex(file),
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
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 0,
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
