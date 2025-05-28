//! Module for page table entries.
//!
//! This module provides a `PageTableEntry` struct, which represents a page table entry,
//! along with functions for creating and manipulating page table entries.

use core::fmt::{self, Debug, Formatter};

use config::mm::PPN_WIDTH;
use mm::address::PhysPageNum;
use polyhal_macro::define_arch_mods;

define_arch_mods!();

/// A page table entry.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PageTableEntry {
    bits: u64,
}

impl PageTableEntry {
    /// Creates a page table entry from the given physical page number and flags.
    pub fn new(ppn: PhysPageNum, flags: PteFlags) -> Self {
        PageTableEntry {
            bits: (ppn.to_usize() as u64) << PPN_OFFSET | flags.bits(),
        }
    }

    /// Returns the physical page number in the page table entry.
    pub fn ppn(self) -> PhysPageNum {
        let ppn_mask = (1 << PPN_WIDTH) - 1;
        let ppn = (self.bits >> PPN_OFFSET) & ppn_mask;
        PhysPageNum::new(ppn as usize)
    }

    /// Returns the flags in the page table entry.
    pub fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.bits)
    }

    /// Sets the physical page number in the page table entry.
    pub fn set_ppn(&mut self, ppn: PhysPageNum) {
        let ppn_mask = ((1 << PPN_WIDTH) - 1) << PPN_OFFSET;
        self.bits = (self.bits & !ppn_mask) | (ppn.to_usize() << PPN_OFFSET) as u64;
    }

    /// Sets the flags in the page table entry.
    pub fn set_flags(&mut self, flags: PteFlags) {
        let flags_mask = PteFlags::all().bits();
        self.bits = (self.bits & !flags_mask) | flags.bits();
    }
}

impl Default for PageTableEntry {
    /// Returns a zeroed page table entry.
    fn default() -> Self {
        PageTableEntry { bits: 0 }
    }
}

impl Debug for PageTableEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageTableEntry")
            .field("ppn", &self.ppn())
            .field("flags", &self.flags())
            .finish()
    }
}
