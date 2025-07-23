use alloc::sync::Arc;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::flags::{AtomicMemfdSeals, MemfdSeals};

pub struct MemInode {
    meta: InodeMeta,
    seals: AtomicMemfdSeals,
    /// if map, record it here
    pseals: AtomicMemfdSeals,
}

impl MemInode {
    pub fn new(seals: MemfdSeals) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            seals: AtomicMemfdSeals::new(seals),
            pseals: AtomicMemfdSeals::new(MemfdSeals::empty()),
        })
    }

    pub fn get_seals(&self) -> MemfdSeals {
        self.seals.load(core::sync::atomic::Ordering::Relaxed)
    }

    pub fn add_seals(&self, seals: MemfdSeals) {
        let mut nseals = self.seals.load(core::sync::atomic::Ordering::Relaxed);
        nseals = nseals | seals;
        self.seals
            .store(nseals, core::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_pseals(&self) -> MemfdSeals {
        self.pseals.load(core::sync::atomic::Ordering::Relaxed)
    }

    pub fn map_seals(&self, seals: MemfdSeals) {
        let mut nseals = self.pseals.load(core::sync::atomic::Ordering::Relaxed);
        nseals = nseals | seals;
        self.pseals
            .store(nseals, core::sync::atomic::Ordering::Relaxed);
    }

    pub fn unmap_seals(&self, seals: MemfdSeals) {
        let mut nseals = self.seals.load(core::sync::atomic::Ordering::Relaxed);
        nseals = nseals & !seals;
        self.seals
            .store(nseals, core::sync::atomic::Ordering::Relaxed);
    }
}

impl Inode for MemInode {
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
            st_nlink: inner.nlink as u32,
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

    fn set_size(&self, size: usize) -> SysResult<()> {
        let nsize = self.size();
        if self.get_seals().contains(MemfdSeals::GROW) && nsize < size {
            return Err(SysError::EPERM);
        }

        self.get_meta().inner.lock().size = size;
        Ok(())
    }
}
