//! Module for address types for Sv39.
//!
//! This module provides types for physical and virtual addresses, as well as
//! physical and virtual page numbers. It also provides functions for converting
//! between these types.

use core::fmt::{self, Debug, Formatter};

use config::mm::{
    KERNEL_MAP_OFFSET, PA_WIDTH_SV39, PAGE_OFFSET_WIDTH, PAGE_SIZE, PPN_WIDTH, USER_END, VPN_WIDTH,
};

#[cfg(target_arch = "riscv64")]
use config::mm::VA_WIDTH_SV39;

/// An address in physical memory defined in Sv39.
///
/// A physical address is a 56-bit integer representing a location in physical
/// memory. The upper 8 bits of the address must be the same as bit 55.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr {
    addr: usize,
}

impl PhysAddr {
    /// Creates a new `PhysAddr` from the given address.
    ///
    /// # Note
    /// We only support physical addresses in the lower half of the address space.
    pub const fn new(addr: usize) -> Self {
        debug_assert!(Self::check_validity(addr));
        PhysAddr { addr }
    }

    /// Checks the validity of the address.
    ///
    /// # Note
    /// MMIO addresses may be outside the RAM range, but they are still valid. This
    /// function is only a sanity check and does not guarantee the address is valid.
    pub const fn check_validity(addr: usize) -> bool {
        let high_bits = addr as isize >> PA_WIDTH_SV39;
        high_bits == 0
    }

    /// Gets the inner `usize` address.
    pub const fn to_usize(self) -> usize {
        self.addr
    }

    /// Gets the offset within the page where the address resides.
    pub const fn page_offset(self) -> usize {
        self.addr % PAGE_SIZE
    }

    /// Gets the page number where the address resides.
    pub const fn page_number(self) -> PhysPageNum {
        PhysPageNum::new(self.addr / PAGE_SIZE)
    }

    /// Rounds the address down to the nearest page boundary.
    pub const fn round_down(self) -> PhysAddr {
        PhysAddr::new(self.addr & !(PAGE_SIZE - 1))
    }

    /// Rounds the address up to the nearest page boundary.
    pub const fn round_up(self) -> PhysAddr {
        PhysAddr::new((self.addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }

    /// Translates a physical address into a virtual address in the kernel space.
    pub const fn to_va_kernel(self) -> VirtAddr {
        VirtAddr::new(self.addr + KERNEL_MAP_OFFSET)
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtAddr {
    addr: usize,
}

impl VirtAddr {
    /// Creates a new `VirtAddr` from the given address.
    ///
    /// # Note for RISC-V
    /// According to the RISC-V Sv39 specification, only the lower 39 bits of
    /// a virtual address are used, and the upper 25 bits must be the same as
    /// bit 38.
    ///
    /// # Note for LoongArch64
    /// LoongArch64 has multiple kinds of virtual addresses: addresses in a
    /// direct mapping configuration window and addresses that are not. The
    /// former kind of addresses may has `0x9` or `0x8` being the upper 4 bits
    /// (in our current implementation), and the latter kind of addresses
    /// is similar to RISC-V.
    pub const fn new(addr: usize) -> Self {
        debug_assert!(Self::check_validity(addr));
        VirtAddr { addr }
    }

    /// Checks the validity of the address.
    #[cfg(target_arch = "riscv64")]
    pub const fn check_validity(addr: usize) -> bool {
        let high_bits = addr as isize >> (VA_WIDTH_SV39 - 1);
        high_bits == 0 || high_bits == -1
    }

    /// Checks the validity of the address.
    ///
    /// # Note for LoongArch64
    /// LoongArch64 has multiple kinds of virtual addresses. This check is only
    /// a sanity check and does not guarantee the address is valid.
    #[cfg(target_arch = "loongarch64")]
    pub const fn check_validity(addr: usize) -> bool {
        let dmw_bits = addr >> 60;
        matches!(dmw_bits, 0x0 | 0xf | 0x8 | 0x9)
    }

    pub const fn in_user_space(self) -> bool {
        self.addr < USER_END
    }

    /// Gets the inner `usize` address.
    pub const fn to_usize(self) -> usize {
        self.addr
    }

    /// Gets the offset within the page where the address resides.
    pub const fn page_offset(self) -> usize {
        self.addr % PAGE_SIZE
    }

    /// Gets the page number where the address resides.
    pub const fn page_number(self) -> VirtPageNum {
        VirtPageNum::new(self.addr / PAGE_SIZE)
    }

    /// Rounds the address down to the nearest page boundary.
    pub const fn round_down(self) -> VirtAddr {
        VirtAddr::new(self.addr & !(PAGE_SIZE - 1))
    }

    /// Rounds the address up to the nearest page boundary.
    pub const fn round_up(self) -> VirtAddr {
        VirtAddr::new((self.addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }

    /// Translates a virtual address into a physical address, if the VA is in the
    /// kernel space.
    pub const fn to_pa_kernel(self) -> PhysAddr {
        PhysAddr::new(self.addr - KERNEL_MAP_OFFSET)
    }
}

impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.addr)
    }
}

/// Physical page number.
///
/// A physical page number is defined as the physical address divided by the
/// page size.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysPageNum {
    page_num: usize,
}

impl PhysPageNum {
    /// Creates a new `PhysPageNum` from the given page number.
    pub const fn new(page_num: usize) -> Self {
        debug_assert!(Self::check_validity(page_num));
        PhysPageNum { page_num }
    }

    /// Checks the validity of the page number.
    pub const fn check_validity(page_num: usize) -> bool {
        let high_bits = page_num >> PPN_WIDTH;
        high_bits == 0
    }

    /// Gets the inner `usize` page number.
    pub const fn to_usize(self) -> usize {
        self.page_num
    }

    /// Gets the starting address of the page.
    pub const fn address(self) -> PhysAddr {
        PhysAddr::new(self.page_num << PAGE_OFFSET_WIDTH)
    }

    /// Translates a physical page number into a virtual page number in the kernel space.
    pub const fn to_vpn_kernel(self) -> VirtPageNum {
        self.address().to_va_kernel().page_number()
    }
}

impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.page_num)
    }
}

/// Virtual page number defined in Sv39.
///
/// A virtual page number is defined as the virtual address divided by the
/// page size.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtPageNum {
    page_num: usize,
}

impl VirtPageNum {
    /// Creates a new `VirtPageNum` from the given page number.
    pub const fn new(page_num: usize) -> Self {
        debug_assert!(Self::check_validity(page_num));
        VirtPageNum { page_num }
    }

    /// Checks the validity of the page number.
    #[cfg(target_arch = "riscv64")]
    pub const fn check_validity(page_num: usize) -> bool {
        let extended_bits = page_num >> VPN_WIDTH;
        extended_bits == 0
    }

    /// Checks the validity of the page number.
    #[cfg(target_arch = "loongarch64")]
    pub const fn check_validity(page_num: usize) -> bool {
        let dmw_bits = page_num >> (VPN_WIDTH - 4);
        matches!(dmw_bits, 0x0 | 0xf | 0x8 | 0x9)
    }

    /// Gets the inner `usize` page number.
    pub const fn to_usize(self) -> usize {
        self.page_num
    }

    /// Gets the starting address of the page.
    pub const fn address(self) -> VirtAddr {
        VirtAddr::new(self.page_num << PAGE_OFFSET_WIDTH)
    }

    /// Translates a virtual page number into a physical page number, if the VPN is in the
    /// kernel space.
    pub const fn to_ppn_kernel(self) -> PhysPageNum {
        self.address().to_pa_kernel().page_number()
    }

    /// Gets a slice pointing to the page.
    ///
    /// # Safety
    /// The caller must ensure that the page is allocated, and the slice should
    /// not outlive the page.
    pub const unsafe fn as_slice(self) -> &'static [u8; PAGE_SIZE] {
        let ptr = self.address().to_usize() as *const [u8; PAGE_SIZE];
        unsafe { &*ptr }
    }

    /// Gets a mutable slice pointing to the page.
    ///
    /// # Safety
    /// The caller must ensure that the page is allocated, and the slice should
    /// not outlive the page.
    pub const unsafe fn as_slice_mut(self) -> &'static mut [u8; PAGE_SIZE] {
        let ptr = self.address().to_usize() as *mut [u8; PAGE_SIZE];
        unsafe { &mut *ptr }
    }

    /// Returns 9-bit indices of the VPN.
    ///
    /// `indices[2]` is the index of the root page table.
    /// `indices[1]` is the index of the second-level page table.
    /// `indices[0]` is the index of the leaf page table.
    pub const fn indices(self) -> [usize; 3] {
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
