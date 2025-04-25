use alloc::sync::Arc;
use config::{device::BLOCK_SIZE, inode::InodeType, vfs::Stat};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    superblock::SuperBlock,
};

pub struct ExeInode {
    meta: InodeMeta,
}

impl ExeInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), super_block),
        });
        inode.set_size(BLOCK_SIZE);
        inode.set_inotype(InodeType::SymLink);
        inode
    }
}

impl Inode for ExeInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
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
