extern crate alloc;
use alloc::sync::Arc;
use config::inode::{InodeMode, InodeType};
use vfs::{
    inode::InodeMeta,
    superblock::{self, SuperBlock},
};
pub struct ExtLinkInode {
    meta: InodeMeta,
}

impl ExtLinkInode {
    pub fn new(target: &str, superblock: Arc<dyn SuperBlock>) -> Self {
        Self {
            meta: InodeMeta::new(
                InodeMode::from_type(InodeType::SymLink),
                superblock.clone(),
                tasget.len,
            ),
        }
    }
}

impl Inode for ExtLinkInode {
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
