pub mod page;

extern crate alloc;
use alloc::{collections::BTreeMap, sync::Arc};
use config::mm::PAGE_SIZE;
use mutex::SpinNoIrqLock;
use page::Page;

/// A page cache that stores pages of a disk file.
#[derive(Debug)]
pub struct PageCache {
    pages: SpinNoIrqLock<BTreeMap<usize, Arc<Page>>>,
}

impl PageCache {
    /// Creates a new page cache.
    pub fn new() -> Self {
        Self {
            pages: SpinNoIrqLock::new(BTreeMap::new()),
        }
    }

    /// Returns the page at the given offset.
    ///
    /// `offset` must be aligned to the page size.
    pub fn get_page(&self, offset: usize) -> Option<Arc<Page>> {
        debug_assert!(offset % PAGE_SIZE == 0);
        self.pages.lock().get(&offset).cloned()
    }

    /// Inserts a page at the given offset.
    /// 
    /// `offset` must be aligned to the page size.
    pub fn insert_page(&self, offset: usize, page: Arc<Page>) {
        debug_assert!(offset % PAGE_SIZE == 0);
        self.pages.lock().insert(offset, page);
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}
