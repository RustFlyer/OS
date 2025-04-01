extern crate alloc;

use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use config::{
    board::BLOCK_SIZE,
    inode::{InodeMode, InodeType},
    vfs::Stat,
};
use lwext4_rust::bindings::ext4_file;
use mutex::{ShareMutex, new_share_mutex};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    superblock::{self, SuperBlock},
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
        Self {
            meta: InodeMeta::new(InodeMode::from_type(InodeType::File), superblock.clone()),
            file: new_share_mutex(file),
        }
    }
}

impl Inode for ExtFileInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino,
            st_mode: self.meta.mode,
            st_nlink: 0,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: self.meta.size.load(Ordering::Relaxed),
            st_blksize: BLOCK_SIZE,
            __pad2: 0,
            st_blocks: (self.meta.size.load(Ordering::Relaxed) / BLOCK_SIZE),
            st_atime: self.meta.time[0],
            st_mtime: self.meta.time[1],
            st_ctime: self.meta.time[2],
            unused: 0,
        })
    }
}
