use alloc::sync::Arc;
use config::inode::InodeType;
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

pub struct FdInfoInode {
    meta: InodeMeta,
    pub thread_id: usize,
    pub file_descriptor: usize,
}

impl FdInfoInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, tid: usize, fd: usize) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), super_block),
            thread_id: tid,
            file_descriptor: fd,
        });
        inode.set_inotype(InodeType::File);
        inode
    }
}

impl Inode for FdInfoInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0, // non-real-file
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
