//! Address types and utilities for Sv39.
//!
//! This module provides types for physical and virtual addresses, as well as
//! physical and virtual page numbers. It also provides functions for converting
//! between these types.

use config::mm::{
    PA_WIDTH_SV39, PAGE_SIZE, PPN_WIDTH_SV39, VA_WIDTH_SV39, VPN_WIDTH_SV39,
};

/// An address in physical memory defined in Sv39.
///
/// A physical address is a 56-bit integer representing a location in physical
/// memory. The upper 8 bits of the address must be the same as bit 55.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr {
    addr: usize,
}

impl PhysAddr {
    /// Creates a new `PhysAddr` from the given address.
    ///
    /// According to the RISC-V Sv39 specification, only the lower 56 bits of
    /// a physical address are used, and the upper 8 bits must be the same as
    /// bit 55.
    ///
    /// # Panics
    ///
    /// This function panics if the upper 8 bits of the address are not the same
    /// as bit 55.
    pub fn new(addr: usize) -> Self {
        let tmp = addr as isize >> PA_WIDTH_SV39;
        debug_assert!(
            tmp == 0 || tmp == -1,
            "invalid physical address: 0x{:x}",
            addr
        );
        PhysAddr { addr }
    }

    /// Gets the inner `usize` address.
    pub fn to_usize(self) -> usize {
        self.addr
    }

    /// Gets the offset within the page where the address resides.
    pub fn page_offset(self) -> usize {
        self.addr % PAGE_SIZE
    }

    /// Gets the page number where the address resides.
    pub fn page_number(self) -> PhysPageNum {
        let ppn_mask = (1 << PPN_WIDTH_SV39) - 1;
        let page_num = (self.addr / PAGE_SIZE) & ppn_mask;
        PhysPageNum::new(page_num)
    }
}

/// An address in virtual memory defined in Sv39.
///
/// A virtual address is a 39-bit integer representing a location in virtual
/// memory. The upper 25 bits of the address must be the same as bit 38.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr {
    addr: usize,
}

impl VirtAddr {
    /// Creates a new `VirtAddr` from the given address.
    ///
    /// According to the RISC-V Sv39 specification, only the lower 39 bits of
    /// a virtual address are used, and the upper 25 bits must be the same as
    /// bit 38.
    ///
    /// # Panics
    ///
    /// This function panics if the upper 25 bits of the address are not the same
    /// as bit 38.
    pub fn new(addr: usize) -> Self {
        let tmp = addr as isize >> VA_WIDTH_SV39;
        debug_assert!(
            tmp == 0 || tmp == -1,
            "invalid virtual address: 0x{:x}",
            addr
        );
        VirtAddr { addr }
    }

    /// Gets the inner `usize` address.
    pub fn to_usize(self) -> usize {
        self.addr
    }

    /// Gets the offset within the page where the address resides.
    pub fn page_offset(self) -> usize {
        self.addr % PAGE_SIZE
    }

    /// Gets the page number where the address resides.
    pub fn page_number(self) -> VirtPageNum {
        let vpn_mask = (1 << VA_WIDTH_SV39) - 1;
        let page_num = (self.addr / PAGE_SIZE) & vpn_mask;
        VirtPageNum { page_num }
    }
}

/// A physical page number defined in Sv39.
///
/// A physical page number is a 44-bit unsigned integer representing the page
/// number of a physical address. The upper 20 bits of the page number must be
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysPageNum {
    page_num: usize,
}

impl PhysPageNum {
    /// Creates a new `PhysPageNum` from the given page number.
    ///
    /// # Panics
    ///
    /// This function panics if the upper 20 bits of the page number
    /// are not zero.
    pub fn new(page_num: usize) -> Self {
        let tmp = page_num >> (64 - PPN_WIDTH_SV39);
        debug_assert!(tmp == 0, "invalid physical page number: 0x{:x}", page_num);
        PhysPageNum { page_num }
    }

    /// Gets the inner `usize` page number.
    pub fn to_usize(self) -> usize {
        self.page_num
    }

    /// Gets the starting address of the page.
    pub fn address(self) -> PhysAddr {
        let addr = self.page_num << (64 - PPN_WIDTH_SV39) >> (64 - PA_WIDTH_SV39);
        PhysAddr::new(addr)
    }
}

/// A virtual page number defined in Sv39.
///
/// A virtual page number is a 39-bit unsized integer representing the page
/// number of a virtual address. The upper 25 bits of the page number is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtPageNum {
    page_num: usize,
}

impl VirtPageNum {
    /// Creates a new `VirtPageNum` from the given page number.
    ///
    /// # Panics
    ///
    /// This function panics if the upper 25 bits of the page number
    /// are not zero.
    pub fn new(page_num: usize) -> Self {
        let tmp = page_num >> (64 - VPN_WIDTH_SV39);
        debug_assert!(tmp == 0, "invalid virtual page number: 0x{:x}", page_num);
        VirtPageNum { page_num }
    }

    /// Gets the inner `usize` page number.
    pub fn to_usize(self) -> usize {
        self.page_num
    }

    /// Gets the starting address of the page.
    pub fn address(self) -> VirtAddr {
        let addr = self.page_num << (64 - VPN_WIDTH_SV39) >> (64 - VA_WIDTH_SV39);
        VirtAddr::new(addr)
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> PhysAddr {
        ppn.address()
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(vpn: VirtPageNum) -> VirtAddr {
        vpn.address()
    }
}
