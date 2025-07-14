use alloc::sync::Arc;
use config::inode::InodeType;
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

use super::MEM_INFO;

pub struct MemInfoInode {
    meta: InodeMeta,
    pub(crate) nodeid: usize,
}

impl MemInfoInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, nodeid: usize) -> Arc<Self> {
        let size = MEM_INFO.lock().serialize_node_meminfo(0).len();
        let inode = Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), super_block),
            nodeid,
        });
        inode.set_size(size);
        inode.set_inotype(InodeType::File);
        inode
    }
}

impl Inode for MemInfoInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = inner.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: self.superblock().dev_id(),
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
