use spin::Once;

use crate::inode::Inode;
extern crate alloc;
use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use config::mm::PAGE_SIZE;
use mm::frame::FrameTracker;
use mutex::SpinNoIrqLock;

type Page = FrameTracker;

pub struct Inopages {
    inode: Once<Weak<dyn Inode>>,
    pages: SpinNoIrqLock<BTreeMap<usize, Arc<Page>>>,
}

impl Inopages {
    pub fn new() -> Self {
        Self {
            inode: Once::new(),
            pages: SpinNoIrqLock::new(BTreeMap::new()),
        }
    }

    pub fn set_inode(&self, inode: Arc<dyn Inode>) {
        self.inode.call_once(|| Arc::downgrade(&inode));
    }

    pub fn get_page(&self, offset: usize) -> Option<Arc<Page>> {
        debug_assert!(is_aligned(offset));
        self.pages.lock().get(&offset).cloned()
    }

    pub fn insert_page(&self, offset: usize, page: Page) {
        debug_assert!(is_aligned(offset));
        self.pages.lock().insert(offset, Arc::new(page));
    }
}

pub fn is_aligned(offset: usize) -> bool {
    offset & (PAGE_SIZE - 1) == 0
}
