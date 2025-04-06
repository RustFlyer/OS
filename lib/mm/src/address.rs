//! Module for address types for Sv39.
//!
//! This module provides types for physical and virtual addresses, as well as
//! physical and virtual page numbers. It also provides functions for converting
//! between these types.

use core::fmt::{self, Debug, Formatter};

use config::mm::{
    KERNEL_MAP_OFFSET, PA_WIDTH_SV39, PAGE_SIZE, PPN_WIDTH_SV39, USER_END, VA_WIDTH_SV39,
    VPN_WIDTH_SV39,
};
use log::{info, warn};
use simdebug::stop;

/// An address in physical memory defined in Sv39.
///
/// A physical address is a 56-bit integer representing a location in physical
/// memory. The upper 8 bits of the address must be the same as bit 55.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        debug_assert!(
            Self::check_validity(addr),
            "invalid physical address: {:#x}",
            addr
        );
        PhysAddr { addr }
    }

    /// Checks the validity of the address.
    pub fn check_validity(addr: usize) -> bool {
        let tmp = addr as isize >> PA_WIDTH_SV39;
        tmp == 0 || tmp == -1
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

    /// Rounds the address down to the nearest page boundary.
    pub fn round_down(self) -> PhysAddr {
        PhysAddr::new(self.addr & !(PAGE_SIZE - 1))
    }

    /// Rounds the address up to the nearest page boundary.
    pub fn round_up(self) -> PhysAddr {
        PhysAddr::new((self.addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }

    /// Translates a physical address into a virtual address in the kernel space.
    pub fn to_va_kernel(self) -> VirtAddr {
        let va = self.addr + KERNEL_MAP_OFFSET;
        VirtAddr::new(va)
    }
}

impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.addr)
    }
}

/// An address in virtual memory defined in Sv39.
///
/// A virtual address is a 39-bit integer representing a location in virtual
/// memory. The upper 25 bits of the address must be the same as bit 38.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        debug_assert!(
            Self::check_validity(addr),
            "invalid virtual address: {:#x}",
            addr
        );
        VirtAddr { addr }
    }

    pub fn check_validity(addr: usize) -> bool {
        let tmp = addr as isize >> VA_WIDTH_SV39;
        tmp == 0 || tmp == -1
    }

    pub fn in_user_space(self) -> bool {
        self.addr < USER_END
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
        let vpn_mask = (1 << VPN_WIDTH_SV39) - 1;
        let page_num = (self.addr / PAGE_SIZE) & vpn_mask;
        VirtPageNum::new(page_num)
    }

    /// Rounds the address down to the nearest page boundary.
    pub fn round_down(self) -> VirtAddr {
        VirtAddr::new(self.addr & !(PAGE_SIZE - 1))
    }

    /// Rounds the address up to the nearest page boundary.
    pub fn round_up(self) -> VirtAddr {
        VirtAddr::new((self.addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }

    /// Translates a virtual address into a physical address, if the VA is in the
    /// kernel space.
    pub fn to_pa_kernel(self) -> PhysAddr {
        // stop();
        info!("{:#x} - {:#x}", self.addr, KERNEL_MAP_OFFSET);
        let pa = self.addr - KERNEL_MAP_OFFSET;
        PhysAddr::new(pa)
    }
}

impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.addr)
    }
}

/// A physical page number defined in Sv39.
///
/// A physical page number is a 44-bit unsigned integer representing the page
/// number of a physical address. The upper 20 bits of the page number must be
/// zero.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        debug_assert!(
            Self::check_validity(page_num),
            "invalid physical page number: {:#x}",
            page_num
        );
        PhysPageNum { page_num }
    }

    /// Checks the validity of the page number.
    pub fn check_validity(page_num: usize) -> bool {
        let tmp = page_num >> PPN_WIDTH_SV39;
        tmp == 0
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

    /// Translates a physical page number into a virtual page number in the kernel space.
    pub fn to_vpn_kernel(self) -> VirtPageNum {
        self.address().to_va_kernel().page_number()
    }
}

impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.page_num)
    }
}

/// A virtual page number defined in Sv39.
///
/// A virtual page number is a 39-bit unsized integer representing the page
/// number of a virtual address. The upper 25 bits of the page number is zero.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        debug_assert!(
            Self::check_validity(page_num),
            "invalid virtual page number: {:#x}",
            page_num
        );
        VirtPageNum { page_num }
    }

    /// Checks the validity of the page number.
    pub fn check_validity(page_num: usize) -> bool {
        let tmp = page_num >> VPN_WIDTH_SV39;
        tmp == 0
    }

    /// Gets the inner `usize` page number.
    pub fn to_usize(self) -> usize {
        self.page_num
    }

    /// Gets the starting address of the page.
    pub fn address(self) -> VirtAddr {
        let addr =
            ((self.page_num as isize) << (64 - VPN_WIDTH_SV39) >> (64 - VA_WIDTH_SV39)) as usize;
        VirtAddr::new(addr)
    }

    /// Gets a slice pointing to the page.
    ///
    /// # Safety
    /// The caller must ensure that the page is allocated, and the slice should
    /// not outlive the page.
    pub unsafe fn as_slice(self) -> &'static [u8; PAGE_SIZE] {
        let ptr = self.address().to_usize() as *const [u8; PAGE_SIZE];
        unsafe { &*ptr }
    }

    /// Gets a mutable slice pointing to the page.
    ///
    /// # Safety
    /// The caller must ensure that the page is allocated, and the slice should
    /// not outlive the page.
    pub unsafe fn as_slice_mut(self) -> &'static mut [u8; PAGE_SIZE] {
        let ptr = self.address().to_usize() as *mut [u8; PAGE_SIZE];
        unsafe { &mut *ptr }
    }

    /// Translates a virtual page number into a physical page number, if the VPN is in the
    /// kernel space.
    pub fn to_ppn_kernel(self) -> PhysPageNum {
        self.address().to_pa_kernel().page_number()
    }

    /// Returns 9-bit indices of the VPN.
    ///
    /// `indices[2]` is the index of the root page table.
    /// `indices[1]` is the index of the second-level page table.
    /// `indices[0]` is the index of the leaf page table.
    pub fn indices(self) -> [usize; 3] {
        let index_mask = 0x1ff;
        let vpn = self.to_usize();
        let mut indices = [0; 3];
        indices[0] = vpn & index_mask;
        indices[1] = (vpn >> 9) & index_mask;
        indices[2] = (vpn >> 18) & index_mask;
        indices
    }
}

impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.page_num)
    }
}

impl From<PhysAddr> for usize {
    fn from(pa: PhysAddr) -> usize {
        pa.to_usize()
    }
}

impl From<VirtAddr> for usize {
    fn from(va: VirtAddr) -> usize {
        va.to_usize()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> PhysAddr {
        ppn.address()
    }
}

impl From<PhysPageNum> for usize {
    fn from(ppn: PhysPageNum) -> usize {
        ppn.to_usize()
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(vpn: VirtPageNum) -> VirtAddr {
        vpn.address()
    }
}

impl From<VirtPageNum> for usize {
    fn from(vpn: VirtPageNum) -> usize {
        vpn.to_usize()
    }
}
