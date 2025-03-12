//! Module for manipulating page tables and manage memory mappings.
//!
//! This module provides a `PageTable` struct that manipulates page tables in momery
//! and tracking allocated pages.

use alloc::vec::Vec;
use config::mm::PTE_PER_TABLE;
use systype::SysResult;

use crate::{
    address::{PhysPageNum, VirtPageNum},
    frame::FrameTracker,
};

use super::pte::{PageTableEntry, PteFlags};

/// A data structure for manipulating page tables and manage memory mappings.
///
/// This struct represents a page table (with its sub page tables) in the memory.
/// It provides methods to manipulate page tables and page table entries in a
/// convenient way. It also tracks allocated frames for the page table itself
/// and allocatable frames mapped by the page table.
///
/// A page table consists of page tables and mapped physical frames. Both of them
/// can be separated into two categories: kernel-used and user-used. Kernel-used
/// tables and frames are shared among all processes, and is not tracked by
/// this struct. User-used tables are exclusively allocated for each process,
/// and are tracked by this struct. When a `PageTable` is dropped, all user-used
/// tables are dropped.
pub struct PageTable {
    /// Physical page number of the root page table.
    root: PhysPageNum,
    /// Frames allocated for user-used tables
    frames: Vec<FrameTracker>,
}

impl PageTable {
    /// Builds an empty page table.
    ///
    /// # Errors
    /// Returns [`AllocError::OutOfMemory`] if there are no free frames.
    pub fn build_empty() -> SysResult<Self> {
        let root_frame = FrameTracker::new()?;
        // SAFETY: the frame is newly allocated for the root page table.
        unsafe {
            PageTableMem::new(root_frame.as_ppn()).clear();
        }
        Ok(PageTable {
            root: root_frame.as_ppn(),
            frames: vec![root_frame],
        })
    }

    /// Gets the physical page number of the root page table.
    pub fn root(&self) -> PhysPageNum {
        self.root
    }

    /// Adds a `FrameTracker` to the page table so that the frame can be deallocated
    /// when the `PageTable` is dropped. Any page table frame exclusive to the page table
    /// must be tracked by calling this method.
    pub fn track_frame(&mut self, frame: FrameTracker) {
        self.frames.push(frame);
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// This method sets any non-present intermediate entries and creates intermediate
    /// page tables if necessary. Note that the returned entry may be invalid.
    ///
    /// This function only support 4 KiB pages.
    pub fn find_entry_create(&mut self, vpn: VirtPageNum) -> &mut PageTableEntry {
        let mut ppn = self.root;
        for (i, index) in vpn.indices().into_iter().enumerate() {
            let mut page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry_mut(index);
            if i == 2 {
                return entry;
            }
            if !entry.is_valid() {
                let frame = FrameTracker::new().expect("out of memory");
                *entry = PageTableEntry::new(frame.as_ppn(), PteFlags::V);
                self.track_frame(frame);
            }
            ppn = entry.ppn();
        }
        unreachable!();
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// If any intermediate entry is not present, returns `None`. Note that the returned
    /// entry may be invalid.
    ///
    /// This function only support 4 KiB pages.
    pub fn find_entry(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let mut ppn = self.root;
        for (i, index) in vpn.indices().into_iter().enumerate() {
            let mut page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry_mut(index);
            if i == 2 {
                return Some(entry);
            }
            if !entry.is_valid() {
                return None;
            }
            ppn = entry.ppn();
        }
        unreachable!();
    }

    /// Maps a page by specifying VPN, PPN, and page table entry flags.
    ///
    /// This method does not allocate the frame for the page. It only sets the mapping
    /// in the page table. The caller should allocate a frame is allocated and set the
    /// mapping by calling this method. Be careful that calling this method with an
    /// already mapped `vpn` will overwrite the existing mapping.
    pub fn map_page(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PteFlags) {
        let entry = self.find_entry_create(vpn);
        *entry = PageTableEntry::new(ppn, flags | PteFlags::V);
    }

    /// Unmaps a page by specifying the VPN.
    ///
    /// This method does not deallocate the frame for the page. It only clears the
    /// mapping in the page table. The caller should deallocate the frame and clear
    /// the mapping by calling this method. Calling this method to clear an unmapped
    /// page is safe.
    pub fn unmap_page(&mut self, vpn: VirtPageNum) {
        if let Some(entry) = self.find_entry(vpn) {
            *entry = PageTableEntry::default();
        }
    }

    /// Maps a range of pages by specifying the starting VPN, a slice of PPNs, and
    /// page table entry flags.
    ///
    /// The range is `[start_vpn, start_vpn + ppns.len())`. The range should be valid
    /// and not overlap with existing mappings.
    ///
    /// This methods does not allocate the frames for the pages. It only sets the mappings
    /// in the page table. The caller should allocate frames and set the mappings by calling
    /// this method. Be careful that calling this method with an already mapped `vpn` will
    /// overwrite the existing mapping.
    pub fn map_range(&mut self, start_vpn: VirtPageNum, ppns: &[PhysPageNum], flags: PteFlags) {
        for &ppn in ppns {
            let vpn =
                VirtPageNum::new(start_vpn.to_usize() + (ppn.to_usize() - ppns[0].to_usize()));
            self.map_page(vpn, ppn, flags);
        }
    }

    /// Unmaps a range of pages by specifying the starting VPN and the number of pages.
    ///
    /// The range is `[start_vpn, start_vpn + count)`.
    ///
    /// This method does not deallocate the frames for the pages. It only clears the mappings
    /// in the page table. The caller should deallocate the frames and clear the mappings by
    /// calling this method. Calling this method to clear any unmapped range is safe.
    pub fn unmap_range(&mut self, start_vpn: VirtPageNum, count: usize) {
        for i in 0..count {
            let vpn = VirtPageNum::new(start_vpn.to_usize() + i);
            self.unmap_page(vpn);
        }
    }
}

/// A helper struct for manipulating a page table in memory temporarily.
struct PageTableMem {
    /// Reference to the page table in memory.
    addr: PhysPageNum,
}

impl PageTableMem {
    /// Constructs a new `PageTableMem` from the given physical address.
    ///
    /// # Safety
    /// The given address must point to a valid page table.
    /// The constructed value must not be used after the page table is deallocated.
    unsafe fn new(ppn: PhysPageNum) -> Self {
        PageTableMem { addr: ppn }
    }

    fn as_slice(&self) -> &'static [PageTableEntry; PTE_PER_TABLE] {
        // SAFETY: the page `ppn` points to is a valid page table thus allocated.
        unsafe { &*(self.addr.to_vpn_kernel().as_slice().as_ptr() as *const _) }
    }

    fn as_slice_mut(&mut self) -> &'static mut [PageTableEntry; PTE_PER_TABLE] {
        // SAFETY: the page `ppn` points to is a valid page table thus allocated.
        unsafe { &mut *(self.addr.to_vpn_kernel().as_slice().as_mut_ptr() as *mut _) }
    }

    /// Gets the entry at the given index.
    fn get_entry(&self, index: usize) -> &'static PageTableEntry {
        &self.as_slice()[index]
    }

    /// Gets the entry at the given index mutably.
    fn get_entry_mut(&mut self, index: usize) -> &'static mut PageTableEntry {
        &mut self.as_slice_mut()[index]
    }

    /// Clears the page table.
    fn clear(&mut self) {
        self.as_slice_mut().fill(PageTableEntry::default());
    }
}
