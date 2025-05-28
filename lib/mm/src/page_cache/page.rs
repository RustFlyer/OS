//! Module for managing physical pages.
//!
//! This module defines the [`Page`] struct, which represents a physical page in the system.
//! [`Page`] differs from [`FrameTracker`] in several ways:
//! - [`FrameTracker`] represents a bare physical page, while [`Page`] represents a page that is
//!   associated with a process, a file, and/or a device, and so on.
//! - [`Page`] provides interior mutability, allowing the user to modify the page contents without
//!   taking a mutable reference to the [`Page`]. However, it does not provide any synchronization
//!   mechanism, so the user is responsible for doing so if needed.
//!
//! Note that a page is tracked either by a [`FrameTracker`] or a [`Page`] (which wraps a
//! [`FrameTracker`]). When either of them is dropped, the page is freed. The user has no way
//! to have a page that is tracked by both a [`FrameTracker`] and a [`Page`] at the same time.
//!
//! A [`Page`] is used in the following scenarios:
//! - Processes: A [`Page`] is tracked by a process when it is created for the process. Because of
//!   the mechanism of shared memory, a [`Page`] can be tracked by several processes simultaneously.
//! - Files: A [`Page`] is tracked by a disk file when it is created as a page in the page cache
//!   of the file. Because `mmap` allows a file to be mapped into processes directly, such a
//!   [`Page`] can also be tracked by processes.
//! - Devices: A [`Page`] is tracked by a block device when it is created as a container for blocks
//!   in the buffer cache of the device. Such a [`Page`] is not tracked by any process or file.
//!
//! A [`Page`] also provides a way to find which process, file, or device it is tracked by. This is
//! necessary, because a [`Page`] may be destroyed when it is swapped out or flushed to disk.

use core::cell::SyncUnsafeCell;

use config::mm::PAGE_SIZE;
use systype::error::SysResult;

use crate::{address::PhysPageNum, frame::FrameTracker};

/// A physical page in the system.
///
/// See the module-level documentation for more information.
#[derive(Debug)]
pub struct Page {
    /// The underlying physical page.
    ///
    /// # Note
    /// This is a `SyncUnsafeCell` because we do not care about synchronization
    /// when accessing the page data simultaneously from multiple threads.
    frame: SyncUnsafeCell<FrameTracker>,
    /// Which mapping this page comes from.
    mapping: Mapping,
}

/// The mapping of a page.
///
/// This is used to find out:
/// - VMAs that is sharing this page, if it is an anonymous page.
/// - the inode from which this page is read, if it is a file-backed page.
/// - the buffer heads that is sharing this page, if it is a block device page.
#[derive(Debug)]
pub enum Mapping {
    Anonymous,
    // File(Arc<PageCache>),
    // BlockDevice(Vec<BufferHead>),
}

impl Page {
    /// Creates an anonymous page.
    ///
    /// # Errors
    /// Returns an [`ENOMEM`] error if the allocation fails.
    pub fn build() -> SysResult<Self> {
        Ok(Self {
            frame: SyncUnsafeCell::new(FrameTracker::build()?),
            mapping: Mapping::Anonymous,
        })
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
