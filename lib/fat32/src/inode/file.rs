use alloc::sync::Arc;
use config::inode::InodeType;
use mutex::{ShareMutex, new_share_mutex};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

use crate::FatFile;

pub struct FatFileInode {
    meta: InodeMeta,
    pub file: ShareMutex<FatFile>,
}

impl FatFileInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, file: FatFile) -> Arc<Self> {
        let size = file.size().unwrap() as usize;
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), super_block.clone()),
            file: new_share_mutex(file),
        });
        inode.set_inotype(InodeType::File);
        inode.set_size(size);
        inode
    }
}

impl Inode for FatFileInode {
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
            st_blocks: (len as u64 / 512),
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
