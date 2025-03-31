use core::sync::{self, atomic::AtomicUsize};

use crate::{inoid::alloc_ino, superblock::SuperBlock};
use config::{
    inode::{InodeMode, InodeState, InodeType},
    vfs::{Stat, TimeSpec},
};
use downcast_rs::{Downcast, DowncastSync, impl_downcast};
use mm::vm::page_cache::PageCache;
use mutex::SpinNoIrqLock;

extern crate alloc;
use alloc::sync::{Arc, Weak};

use systype::SysResult;

use core::sync::atomic::Ordering;

pub struct InodeMeta {
    pub ino: usize,
    pub inomode: InodeMode,
    pub page_cache: Option<PageCache>,
    pub superblock: Weak<dyn SuperBlock>,

    pub size: AtomicUsize,
    pub time: [TimeSpec; 3],
    pub inostate: SpinNoIrqLock<InodeState>,
}

impl InodeMeta {
    pub fn new(inomode: InodeMode, superblock: Arc<dyn SuperBlock>, size: usize) -> Self {
        Self {
            ino: alloc_ino(),
            inomode,
            page_cache: None,
            superblock: Arc::downgrade(&superblock),
            size: AtomicUsize::new(size),
            time: [TimeSpec::default(); 3],
            inostate: SpinNoIrqLock::new(InodeState::Init),
        }
    }
}

pub trait Inode: Send + Sync + DowncastSync {
    fn get_meta(&self) -> &InodeMeta;

    fn get_attr(&self) -> SysResult<Stat>;
}

impl dyn Inode {
    pub fn ino(&self) -> usize {
        self.get_meta().ino
    }

    pub fn inotype(&self) -> InodeType {
        self.get_meta().inomode.to_type()
    }

    pub fn pages<'a>(self: &'a Arc<dyn Inode>) -> Option<&'a PageCache> {
        self.get_meta().page_cache.as_ref()
    }

    pub fn size(&self) -> usize {
        self.get_meta().size.load(Ordering::Relaxed)
    }

    pub fn set_size(&self, size: usize) {
        self.get_meta().size.store(size, Ordering::Relaxed);
    }

    pub fn state(&self) -> InodeState {
        *self.get_meta().inostate.lock()
    }

    pub fn set_state(&self, state: InodeState) {
        *self.get_meta().inostate.lock() = state;
    }
}

impl_downcast!(sync Inode);
