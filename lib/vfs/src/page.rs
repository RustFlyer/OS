//! Module for managing physical pages.
//!
//! This module defines the [`Page`] struct, which represents a physical page in the system.
//! [`Page`] differs from [`FrameTracker`] in that [`Page`] is a higher-level abstraction
//! that provides more functionalities and tracks the status of a physical page. For example,
//! a [`Page`] may be associated with a specific file in the filesystem, and may be associated
//! with the buffer cache. In contrast, [`FrameTracker`] only tracks a physical page.
//! If the user wants to manage bare physical pages, such as allocating a page for a page
//! table, they should use [`FrameTracker`] directly.
//!
//! Note that a page is tracked either by a [`FrameTracker`] or a [`Page`] (which wraps a
//! [`FrameTracker`]). When either of them is dropped, the page is freed. The user has no way
//! to have a page that is tracked by both a [`FrameTracker`] and a [`Page`] at the same time.
//!
//! A [`Page`] does not provide any synchronization mechanism, and multiple mutable references
//! to a [`Page`] can be created. The user is responsible for ensuring its thread-safety, if
//! needed.

use core::cell::SyncUnsafeCell;

use config::mm::PAGE_SIZE;
use mm::{address::PhysPageNum, frame::FrameTracker};
use systype::SysResult;

/// A physical page in the system.
///
/// See the module-level documentation for more details.
pub struct Page {
    /// The underlying physical page.
    ///
    /// # Note
    /// This is a `SyncUnsafeCell` because we do not care about synchronization
    /// when accessing the page data simultaneously from multiple threads.
    frame: SyncUnsafeCell<FrameTracker>,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

impl Page {
    /// Allocates a new page and returns a [`Page`] object.
    ///
    /// # Errors
    /// Returns an [`ENOMEM`] error if the allocation fails.
    pub fn build() -> SysResult<Self> {
        Ok(Self {
            frame: SyncUnsafeCell::new(FrameTracker::build()?),
        })
    }

    /// Creates a new [`Page`] from an existing [`FrameTracker`].
    pub fn from_frame(frame: FrameTracker) -> Self {
        Self {
            frame: SyncUnsafeCell::new(frame),
        }
    }

    /// Copies the contents of another [`Page`] into this [`Page`].
    pub fn copy_from_page(&self, another: &Page) {
        let dst = self.as_mut_slice();
        let src = another.as_slice();
        dst.copy_from_slice(src);
    }

    /// Returns the physical page number of the page.
    pub fn ppn(&self) -> PhysPageNum {
        unsafe { self.frame.get().as_ref_unchecked().ppn() }
    }

    /// Returns a reference to the underlying [`FrameTracker`].
    pub fn as_slice(&self) -> &[u8; PAGE_SIZE] {
        unsafe { self.frame.get().as_ref_unchecked().as_slice() }
    }

    /// Returns a mutable reference to the underlying page.
    #[allow(clippy::mut_from_ref)]
    pub fn as_mut_slice(&self) -> &mut [u8; PAGE_SIZE] {
        unsafe { self.frame.get().as_mut_unchecked().as_mut_slice() }
    }
}
