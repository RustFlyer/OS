use alloc::sync::Arc;

use config::{device::BLOCK_SIZE, inode::InodeType};
use systype::error::SysResult;

use crate::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

use super::super::super::FanotifyGroup;

/// Inode for an fanotify group file descriptor.
///
/// This inode corresponds to the special file that is created when `fanotify_init` is
/// called. It provides access to the fanotify group's event queue and allows responses
/// to permission events.
pub struct FanotifyGroupInode {
    meta: InodeMeta,
    /// Reference to the fanotify group that this inode represents.
    group: Arc<FanotifyGroup>,
}

impl FanotifyGroupInode {
    /// Creates a new fanotify group inode.
    pub fn new(superblock: Arc<dyn SuperBlock>, group: Arc<FanotifyGroup>) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), superblock),
            group,
        });

        inode.set_inotype(InodeType::File);
        inode
    }

    /// Gets a reference to the associated fanotify group
    pub fn group(&self) -> &Arc<FanotifyGroup> {
        &self.group
    }
}

impl Inode for FanotifyGroupInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();

        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
            st_nlink: 1,
            st_uid: inner.uid,
            st_gid: inner.gid,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: BLOCK_SIZE as u32,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
