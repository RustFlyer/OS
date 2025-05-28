use alloc::sync::Arc;
use config::inode::InodeMode;
use driver::{CHAR_DEVICE, CharDevice};
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    superblock::SuperBlock,
};

pub struct TtyInode {
    meta: InodeMeta,
    pub char_dev: Arc<dyn CharDevice>,
}

pub fn get_char_device() -> Arc<dyn CharDevice> {
    CHAR_DEVICE.get().unwrap().clone()
}

impl TtyInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let meta = InodeMeta::new(alloc_ino(), super_block);
        meta.inner.lock().mode = InodeMode::CHAR;
        let char_dev = get_char_device();
        Arc::new(Self { meta, char_dev })
    }
}

impl Inode for TtyInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: inner.size as u64,
            st_blksize: 0,
            __pad2: 0,
            st_blocks: 0 as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
