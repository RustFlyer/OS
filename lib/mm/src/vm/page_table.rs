//! Module for manipulating page tables and manage memory mappings.
//!
//! This module provides a `PageTable` struct that manipulates page tables in momery
//! and tracking allocated pages.

use alloc::vec::Vec;
use config::mm::PTE_PER_TABLE;

use crate::{
    address::{PhysPageNum, VirtPageNum},
    frame::FrameTracker,
    mm_error::AllocError,
    vm::pte::PageTableEntry,
};

use super::pte::PteFlags;

/// A data structure for manipulating page tables and manage memory mappings.
///
/// This struct represents a page table (with its sub page tables) in the memory.
/// It not only manages the page table itself, but also tracks the allocated frames
/// where the page table(s) reside so that the frames can be deallocated automatically
/// when the `PageTable` is dropped.
pub struct PageTable {
    /// Physical page number of the root page table.
    root: PhysPageNum,
    /// Exclusively allocated frames for the page table.
    ///
    /// Globally shared page tables are not included in this list.
    frames: Vec<FrameTracker>,
}

impl PageTable {
    /// Builds an empty page table.
    ///
    /// # Errors
    /// Returns `AllocError::OutOfMemory` if there are no free frames.
    pub fn build() -> Result<Self, AllocError> {
        let root_frame = FrameTracker::new().ok_or_else(|| AllocError::OutOfMemory)?;
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
    /// must be added to the page table.
    pub fn track_frame(&mut self, frame: FrameTracker) {
        self.frames.push(frame);
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// This method sets any non-present intermediate entries and creates intermediate
    /// page tables if necessary.
    /// The returned entry may be invalid.
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
    /// If any intermediate entry is not present, returns `None`.
    pub fn find_entry(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let mut ppn = self.root;
        for (i, index) in vpn.indices().into_iter().enumerate() {
            let page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry(index);
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

    /// Maps the given VPN to the given PPN with the given flags.
    ///
    /// # Note
    /// [`PageTable::set_mapping`] and [`PageTable::unset_mapping`] should be used
    /// together with operations such as (de)allocating.
    pub fn set_mapping(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PteFlags) {
        let entry = self.find_entry_create(vpn);
        *entry = PageTableEntry::new(ppn, flags | PteFlags::V);
    }

    /// Unmaps the given VPN.
    ///
    /// # Note
    /// [`PageTable::set_mapping`] and [`PageTable::unset_mapping`] should be used
    /// together with operations such as (de)allocating.
    pub fn unset_mapping(&mut self, vpn: VirtPageNum) {
        let entry = self.find_entry_create(vpn);
        *entry = PageTableEntry::default();
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
