use alloc::{string::String, sync::Arc};

use config::{device::BLOCK_SIZE, inode::InodeType};
use systype::error::SysResult;

use crate::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

use super::super::super::FanotifyEntry;

/// Inode for an fanotify event file.
///
/// This represents a special file that is created for each fanotify event when it's read
/// from the fanotify group file.
pub struct FanotifyEventInode {
    meta: InodeMeta,
    /// Reference to the fanotify entry that generated this event.
    entry: Arc<FanotifyEntry>,
    /// Path to the original monitored file.
    target_path: String,
}

impl FanotifyEventInode {
    /// Creates a new fanotify event inode.
    pub fn new(
        superblock: Arc<dyn SuperBlock>,
        entry: Arc<FanotifyEntry>,
        target_path: String,
    ) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), superblock),
            entry,
            target_path,
        });

        inode.set_inotype(InodeType::File);

        inode
    }

    /// Gets a reference to the associated fanotify entry.
    pub fn entry(&self) -> Arc<FanotifyEntry> {
        Arc::clone(&self.entry)
    }

    /// Gets the target path for this fanotify event file.
    pub fn target_path(&self) -> &str {
        &self.target_path
    }
}

impl Inode for FanotifyEventInode {
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
            st_nlink: inner.nlink as u32,
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
