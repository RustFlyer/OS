//! Module for page table entries.
//!
//! This module provides the `Pte` type, which represents a page table entry,
//! along with functions for creating and manipulating page table entries.

use bitflags::bitflags;

use config::mm::PPN_WIDTH_SV39;

use crate::address::PhysPageNum;

/// Offset of the physical page number in a page table entry. A physical page
/// number located at bits 10-53 in a page table entry.
const PPN_OFFSET: usize = 10;

bitflags! {
    /// Flags for a page table entry.
    ///
    /// The flags are defined in the RISC-V Sv39 specification as follows:
    ///
    /// - `V`: Valid. When set, the PTE is valid. If one of the R, W, or X bits
    ///   is set, the PTE points to a physical page. Otherwise, the PTE points
    ///   to a next-level page table.
    /// - `R`: Read. If set, the page pointed at by the PTE is readable.
    /// - `W`: Write. If set, the page pointed at by the PTE is writable.
    /// - `X`: Execute. If set, the page pointed at by the PTE is executable.
    /// - `U`: User. If set, the page pointed at by the PTE is accessible in
    ///   user mode.
    /// - `G`: Global. If set, the address range pointed at by the PTE is global
    ///   mapped, which is in all address spaces.
    /// - `A`: Accessed. If set, the page pointed at by the PTE has been
    ///   accessed.
    /// - `D`: Dirty. If set, the page pointed at by the PTE has been written to.
    ///
    /// Flag `RSW` is reserved for supervisor software, but we do not use it in
    /// the current implementation.
    #[derive(Debug, Clone, Copy)]
    pub struct PteFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

/// A page table entry defined in Sv39.
///
/// A page table entry is a 64-bit data structure that represents a mapping
/// from a virtual page number to a physical page number.
///
/// The lower 8 bits of an entry are flags, bits 10-53 are the physical page
/// number, and the upper 10 bits are reserved for extensions. In our
/// implementation, the upper 10 bits are always zero.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry {
    bits: usize,
}

impl PageTableEntry {
    /// Creates a page table entry from the given physical page number and flags.
    pub fn new(ppn: PhysPageNum, flags: PteFlags) -> Self {
        PageTableEntry {
            bits: ppn.to_usize() << PPN_OFFSET | flags.bits() as usize,
        }
    }

    /// Returns the physical page number in the page table entry.
    pub fn ppn(self) -> PhysPageNum {
        let ppn_mask = (1 << PPN_WIDTH_SV39) - 1;
        let ppn = (self.bits >> PPN_OFFSET) & ppn_mask;
        PhysPageNum::new(ppn)
    }

    /// Returns the flags in the page table entry.
    pub fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.bits as u8)
    }

    /// Sets the physical page number in the page table entry.
    pub fn set_ppn(&mut self, ppn: PhysPageNum) {
        let ppn_mask = ((1 << PPN_WIDTH_SV39) - 1) << PPN_OFFSET;
        self.bits = (self.bits & !ppn_mask) | (ppn.to_usize() << PPN_OFFSET);
    }

    /// Sets the flags in the page table entry.
    pub fn set_flags(&mut self, flags: PteFlags) {
        let flags_mask = PteFlags::all().bits() as usize;
        self.bits = (self.bits & !flags_mask) | flags.bits() as usize;
    }

    /// Returns whether the page is valid.
    pub fn is_valid(self) -> bool {
        self.flags().contains(PteFlags::V)
    }

    /// Returns whether the page is readable.
    pub fn is_readable(self) -> bool {
        self.flags().contains(PteFlags::R)
    }

    /// Returns whether the page is writable.
    pub fn is_writable(self) -> bool {
        self.flags().contains(PteFlags::W)
    }

    /// Returns whether the page is executable.
    pub fn is_executable(self) -> bool {
        self.flags().contains(PteFlags::X)
    }

    /// Returns whether the page is accessible in user mode.
    pub fn is_user(self) -> bool {
        self.flags().contains(PteFlags::U)
    }

    /// Returns whether the page is global mapped.
    pub fn is_global(self) -> bool {
        self.flags().contains(PteFlags::G)
    }

    /// Returns whether the page has been accessed.
    pub fn is_accessed(self) -> bool {
        self.flags().contains(PteFlags::A)
    }

    /// Returns whether the page has been written to.
    pub fn is_dirty(self) -> bool {
        self.flags().contains(PteFlags::D)
    }
}

impl Default for PageTableEntry {
    /// Returns a default page table entry which is invalid (unmapped).
    fn default() -> Self {
        PageTableEntry { bits: 0 }
    }
}
