//! Module for managing virtual memory areas.
//!
//! A virtual memory area (VMA) is a contiguous region of virtual memory in an
//! address space, whose pages have a common set of attributes, such as permissions
//! and mapping type. A VMA is represented by a [`VmArea`] struct, which tracks the
//! physical pages allocated in the VMA.
//!
//! We documented in [`crate::frame`] that there are two types of frames: allocatable
//! frames and kernel frames. Allocatable frames are managed by [`VmArea`]. Kernel
//! frames are not managed by [`VmArea`] because they do not need allocating and
//! deallocating. [`VmArea`] only manages the physical pages allocated for user-used
//! tables and frames.
//!
//! We implement modularized page fault handling as follows:
//! - A [`VmArea`] struct has common fields for all typed of VMAs, as well as unique
//!   fields for a specific type of VMA represented by a [`TypedArea`] enum.
//! - A [`VmArea`] struct is registered with a page fault handler function for the VMA's
//!   specific type when constructed via calling a `new_*` constructor corresponding to
//!   that type.
//! - The `fault_handler` method of a specific type of VMA is finally responsible for
//!   handling the page fault.
//!
//! With this design, we avoid using trait objects or an enum for VMA types, while still
//! maintaining modularization and extensibility.

use alloc::vec::Vec;
use core::fmt::Debug;
use systype::{SysError, SysResult};

use bitflags::bitflags;

use super::{page_table::PageTable, pte::PteFlags};
use crate::{
    address::{PhysPageNum, VirtAddr},
    frame::FrameTracker,
};

bitflags! {
    /// Memory permission corresponding to R, W, X, and U bits in a page table entry.
    ///
    /// The bits of `MemPerm` are a subset of the bits of `PteFlags`, and their bit
    /// positions are the same as those in `PteFlags` for easy conversion between them.
    ///
    /// Do not set any unknown bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MemPerm: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

impl MemPerm {
    /// Create a new `MemPerm` from a set of `PteFlags`.
    pub fn from(flags: PteFlags) -> Self {
        Self::from_bits_truncate(flags.bits())
    }
}

impl PteFlags {
    /// Create a new `PteFlags` from a set of `MemPerm`.
    ///
    /// When `MemPerm` does not contain `U`, `G` is set in the returned `PteFlags`.
    pub fn from(perm: MemPerm) -> Self {
        let mut flags = Self::from_bits_retain(perm.bits());
        if !perm.contains(MemPerm::U) {
            flags |= Self::G;
        }
        flags
    }
}

/// Data passed to a page fault handler.
///
/// This struct is used to pass data to a page fault handler registered in a [`VmArea`].
#[derive(Debug)]
pub(crate) struct PageFaultInfo<'a> {
    /// Faulting virtual address.
    pub fault_addr: VirtAddr,
    /// Page table.
    pub page_table: &'a mut PageTable,
    /// How the address was accessed when the fault occurred.
    pub access: MemPerm,
}

/// A virtual memory area (VMA).
///
/// A VMA is a contiguous region of virtual memory in an address space that has
/// a common set of attributes, such as permissions and mapping type.
#[derive(Debug)]
pub struct VmArea {
    /// Starting virtual address.
    start_va: VirtAddr,
    /// Ending virtual address (exclusive).
    end_va: VirtAddr,
    /// Page table entry flags.
    flags: PteFlags,
    /// Permission.
    perm: MemPerm,
    /// Unique data of a specific type of VMA.
    map_type: TypedArea,
    /// Page fault handler.
    handler: Option<fn(&mut Self, PageFaultInfo) -> SysResult<()>>,
}

impl VmArea {
    /// Constructs a [`VmArea`] whose specific type is [`TypedArea::Kernel`].
    pub(crate) fn new_kernel(start_va: VirtAddr, end_va: VirtAddr, flags: PteFlags) -> Self {
        Self {
            start_va,
            end_va,
            flags,
            perm: MemPerm::from(flags),
            map_type: TypedArea::Kernel(KernelArea),
            handler: None,
        }
    }

    /// Constructs a [`VmArea`] whose specific type is [`TypedArea::MemoryBacked`].
    pub fn new_memory_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: PteFlags,
        memory: &'static [u8],
    ) -> Self {
        Self {
            start_va,
            end_va,
            flags,
            perm: MemPerm::from(flags),
            map_type: TypedArea::MemoryBacked(MemoryBackedArea::new(memory)),
            handler: Some(MemoryBackedArea::fault_handler),
        }
    }

    /// Handles a page fault happened in this VMA.
    ///
    /// # Errors
    /// Returns [`SysError::EFAULT`] if the access permission is not allowed.
    pub(crate) fn handle_page_fault(&mut self, info: PageFaultInfo) -> SysResult<()> {
        if let Some(handler) = self.handler {
            handler(self, info)
        } else {
            panic!("page fault handler: handler not registered");
        }
    }

    pub fn contains(&self, va: VirtAddr) -> bool {
        va >= self.start_va && va < self.end_va
    }

    /// Returns the starting virtual address of the VMA.
    pub fn start_va(&self) -> VirtAddr {
        self.start_va
    }

    /// Returns the ending virtual address of the VMA.
    pub fn end_va(&self) -> VirtAddr {
        self.end_va
    }
}

/// Unique data of a specific type of VMA. This enum is used in [`VmArea`].
#[derive(Debug)]
pub(crate) enum TypedArea {
    /// A helper VMA representing one in the kernel space.
    Kernel(KernelArea),
    /// A memory-backed VMA.
    MemoryBacked(MemoryBackedArea),
    /// A file-backed VMA.
    ///
    /// A file-backed VMA is backed by a file. It is used for memory-mapped files.
    FileBacked,
    /// An anonymous VMA.
    ///
    /// An anonymous VMA is not backed by any file or memory. It is used for stack
    /// and heap.
    Anonymous,
}

/// A helper VMA representing one in the kernel space.
///
/// This struct is used to map an area in the kernel space to the kernel page table.
/// It is of no use after the kernel page table is set up.
///
/// A kernel area must be aligned to the size of a page.
#[derive(Debug)]
pub(crate) struct KernelArea;

impl KernelArea {
    /// Maps the kernel area to the kernel page table.
    pub fn map(area: &VmArea, page_table: &mut PageTable) {
        match area.map_type {
            TypedArea::Kernel(_) => {}
            _ => panic!("KernelArea::map: not a kernel area"),
        }

        let &VmArea {
            start_va,
            end_va,
            flags,
            ..
        } = area;

        let start_vpn = start_va.page_number();
        let end_vpn = end_va.round_up().page_number();
        let start_ppn = start_vpn.to_ppn_kernel().to_usize();
        let end_ppn = end_vpn.to_ppn_kernel().to_usize();
        let ppns = (start_ppn..end_ppn)
            .map(PhysPageNum::new)
            .collect::<Vec<_>>();

        page_table.map_range(start_vpn, &ppns, flags);
    }
}

/// A memory-backed VMA.
///
/// This is a type for debugging purposes. A memory-backed VMA takes a slice of
/// memory as its backing store.
#[derive(Debug)]
pub(crate) struct MemoryBackedArea {
    /// The memory backing store.
    pub memory: &'static [u8],
    /// Allocated physical pages.
    pub pages: Vec<FrameTracker>,
}

impl MemoryBackedArea {
    /// Creates a new memory-backed VMA.
    fn new(memory: &'static [u8]) -> Self {
        Self {
            memory,
            pages: Vec::new(),
        }
    }

    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        // Extract data needed for fault handling.
        let &mut Self {
            memory,
            ref mut pages,
        } = match &mut area.map_type {
            TypedArea::MemoryBacked(memory_backed) => memory_backed,
            _ => panic!("fault_handler: not a memory-backed area"),
        };

        let &mut VmArea {
            start_va,
            end_va,
            flags,
            perm,
            ..
        } = area;

        let PageFaultInfo {
            fault_addr,
            page_table,
            access,
        } = info;

        // Check permission.
        if !access.contains(perm) {
            return Err(SysError::EFAULT);
        }

        // Allocate a frame and map the page.
        let mut frame = FrameTracker::new()?;
        page_table.map_page(fault_addr.page_number(), frame.as_ppn(), flags);

        // Copy data from the memory backing store to the allocated frame.
        let fill_va_start = VirtAddr::max(start_va, fault_addr.round_down());
        let fill_va_end = VirtAddr::min(end_va, fault_addr.round_up());
        let fill_len = fill_va_end.to_usize() - fill_va_start.to_usize();
        let page_offset = fill_va_start.to_usize() - fault_addr.round_down().to_usize();
        let area_offset = fill_va_start.to_usize() - start_va.to_usize();

        let memory_to_copy = &memory[area_offset..area_offset + fill_len];
        let memory_to_fill = &mut frame.as_slice_mut()[page_offset..page_offset + fill_len];
        memory_to_fill.copy_from_slice(memory_to_copy);

        // Track the allocated frame.
        pages.push(frame);

        // Flush TLB.
        riscv::asm::sfence_vma(page_table.root().to_usize(), fault_addr.to_usize());

        Ok(())
    }
}
