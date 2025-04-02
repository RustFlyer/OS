extern crate alloc;
use core::sync::atomic::Ordering;

use alloc::sync::Arc;

use config::{
    board::BLOCK_SIZE,
    inode::{InodeMode, InodeType},
    vfs::Stat,
};
use lwext4_rust::bindings::ext4_dir;
use mutex::{ShareMutex, new_share_mutex};
use vfs::{
    file::FileMeta,
    inode::{Inode, InodeMeta},
    superblock::{self, SuperBlock},
};

use crate::{dentry::ExtDentry, ext::dir::ExtDir};

pub struct ExtDirInode {
    meta: InodeMeta,
    pub(crate) dir: ShareMutex<ExtDir>,
}

unsafe impl Send for ExtDirInode {}
unsafe impl Sync for ExtDirInode {}

impl ExtDirInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, dir: ExtDir) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(InodeMode::from_type(InodeType::Dir), superblock.clone(), 0),
            dir: new_share_mutex(dir),
        })
    }
}

impl Inode for ExtDirInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<config::vfs::Stat> {
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino,
            st_mode: self.meta.inomode.bits(),
            st_nlink: 0, //todo!
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
