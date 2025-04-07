use alloc::sync::Arc;

use config::{device::BLOCK_SIZE, vfs::Stat};
use mutex::{ShareMutex, new_share_mutex};
use systype::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    superblock::SuperBlock,
};

use crate::ext::file::ExtFile;

pub struct ExtFileInode {
    meta: InodeMeta,
    pub(crate) file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileInode {}
unsafe impl Sync for ExtFileInode {}

impl ExtFileInode {
    pub fn new(superblock: Arc<dyn SuperBlock>, mut file: ExtFile) -> Arc<Self> {
        let fsize = file.size();
        let meta = InodeMeta::new(0, superblock);
        meta.inner.lock().size = fsize as usize;
        Arc::new(Self {
            meta,
            file: new_share_mutex(file),
        })
    }
}

impl Inode for ExtFileInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: self.meta.inner.lock().mode.bits(),
            st_nlink: 0,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: self.meta.inner.lock().size as u64,
            st_blksize: BLOCK_SIZE as u32,
            __pad2: 0,
            st_blocks: (self.meta.inner.lock().size / BLOCK_SIZE) as u64,
            st_atime: self.meta.inner.lock().atime,
            st_mtime: self.meta.inner.lock().mtime,
            st_ctime: self.meta.inner.lock().ctime,
            unused: 0,
        })
    }
}
