pub mod page;

use alloc::{collections::BTreeMap, sync::Arc};

use config::mm::PAGE_SIZE;
use mutex::SpinNoIrqLock;
use page::Page;
use systype::error::SysResult;

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

    /// Returns the page at the given file offset.
    ///
    /// `offset` must be aligned to the page size.
    pub fn get_page(&self, offset: usize) -> Option<Arc<Page>> {
        debug_assert!(offset % PAGE_SIZE == 0);
        self.pages.lock().get(&offset).cloned()
    }

    /// Inserts a page at the given file offset.
    ///
    /// `offset` must be aligned to the page size.
    pub fn insert_page(&self, offset: usize, page: Arc<Page>) {
        debug_assert!(offset % PAGE_SIZE == 0);
        self.pages.lock().insert(offset, page);
    }

    /// Creates a new page at the given file offset in the page cache, copies data from
    /// `data` into the page starting at the given page offset, and returns the page.
    ///
    /// The memory range from `page_offset` to `page_offset + data.len()` must be within
    /// the bounds of the page. Memory outside this range will be filled with zeroes.
    ///
    /// Pass an empty slice in `data` and 0 in `page_offset` to create a zeroed page.
    ///
    /// `offset` must be aligned to the page size.
    pub fn create_page(&self, offset: usize, data: &[u8], page_offset: usize) -> SysResult<Arc<Page>> {
        debug_assert!(offset % PAGE_SIZE == 0);
        debug_assert!(page_offset + data.len() <= PAGE_SIZE);

        let page = Arc::new(Page::build()?);
        let page_slice = page.as_mut_slice();
        page_slice[..page_offset].fill(0);
        page_slice[page_offset..page_offset + data.len()]
            .copy_from_slice(data);
        page_slice[page_offset + data.len()..].fill(0);

        self.insert_page(offset, page.clone());
        Ok(page)
    }

    /// Creates a new zeroed page at the given file offset in the page cache, and returns
    /// the page.
    ///
    /// This methods is equivalent to calling `create_page(offset, &[], 0)`.
    ///
    /// `offset` must be aligned to the page size.
    pub fn create_zeroed_page(&self, offset: usize) -> SysResult<Arc<Page>> {
        self.create_page(offset, &[], 0)
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}
