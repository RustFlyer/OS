use alloc::sync::Arc;
use config::{
    device::BLOCK_SIZE,
    inode::{InodeMode, InodeType},
};
use systype::error::SysResult;
use vfs::{
    file::File,
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

pub struct LoopInode {
    meta: InodeMeta,
    pub minor: u32,
    pub file: Arc<dyn File>,
}

impl LoopInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, minor: u32, file: Arc<dyn File>) -> Arc<Self> {
        let size = 0x100000;
        let mode = InodeMode::BLOCK;
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), superblock),
            minor,
            file,
        });
        inode.set_inotype(InodeType::from(mode));
        inode.set_size(size);
        inode
    }
}

impl Inode for LoopInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();
        let size = inner.size;
        Ok(Stat {
            st_dev: (7 << 8) | self.minor as u64,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: size as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (size / BLOCK_SIZE) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
