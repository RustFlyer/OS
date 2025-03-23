use core::sync::atomic::AtomicUsize;

use crate::inopage::Inopages;
use config::{
    inode::{InodeMode, InodeState, InodeType},
    vfs::{Stat, TimeSpec},
};
use downcast_rs::{Downcast, impl_downcast};
use mutex::SpinNoIrqLock;

extern crate alloc;
use alloc::sync::Arc;

use systype::SysResult;

use core::sync::atomic::Ordering;

pub struct InodeMeta {
    pub ino: usize,
    pub inomode: InodeMode,
    pub inopages: Option<Inopages>,
    pub superblock: usize,

    pub size: AtomicUsize,
    pub time: [TimeSpec; 3],
    pub inostate: SpinNoIrqLock<InodeState>,
}

pub trait Inode: Send + Sync + Downcast {
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

    pub fn pages<'a>(self: &'a Arc<dyn Inode>) -> Option<&'a Inopages> {
        self.get_meta().inopages.as_ref().map(|a| {
            a.set_inode(self.clone());
            a
        })
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

impl_downcast!(Inode);
