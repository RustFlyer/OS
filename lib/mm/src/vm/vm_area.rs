//! A module for managing virtual memory areas.
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

use alloc::{slice, vec::Vec};
use config::mm::PAGE_SIZE;
use core::fmt::Debug;
use systype::{SysError, SysResult};

use bitflags::bitflags;

use super::{page_table::PageTable, pte::PteFlags};
use crate::{address::VirtAddr, frame::FrameTracker};

bitflags! {
    /// Memory permission corresponding to R, W, X, and U bits in a page table entry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MemPerm: u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const X = 1 << 2;
        const U = 1 << 3;
    }
}

impl From<MemPerm> for PteFlags {
    /// Convert `MemPerm` to `PteFlags`.
    fn from(perm: MemPerm) -> Self {
        let mut flags = Self::empty();
        if perm.contains(MemPerm::U) {
            flags |= PteFlags::U;
        } else {
            flags |= PteFlags::G;
        }
        if perm.contains(MemPerm::R) {
            flags |= PteFlags::R;
        }
        if perm.contains(MemPerm::W) {
            flags |= PteFlags::W;
        }
        if perm.contains(MemPerm::X) {
            flags |= PteFlags::X;
        }
        flags
    }
}

/// Data passed to a page fault handler.
///
/// This struct is used to pass data to a page fault handler registered in a [`VmArea`].
#[derive(Debug)]
pub struct PageFaultInfo<'a> {
    /// Faulting virtual address.
    pub addr: VirtAddr,
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
    pub start_va: VirtAddr,
    /// Ending virtual address (exclusive).
    pub end_va: VirtAddr,
    /// Page table entry flags.
    pub flags: PteFlags,
    /// Permission.
    pub perm: MemPerm,
    /// Unique data of a specific type of VMA.
    pub map_type: TypedArea,
    /// Page fault handler.
    pub handler: Option<fn(&mut Self, PageFaultInfo) -> SysResult<()>>,
}

impl VmArea {
    /// Constructs a [`VmArea`] whose specific type is [`TypedArea::MemoryBacked`].
    pub fn new_memory_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: PteFlags,
        perm: MemPerm,
        memory: &'static [u8],
    ) -> Self {
        Self {
            start_va,
            end_va,
            flags,
            perm,
            map_type: TypedArea::MemoryBacked(MemoryBackedArea::new(memory)),
            handler: Some(MemoryBackedArea::fault_handler),
        }
    }

    /// Handles a page fault happened in this VMA.
    pub fn handle_page_fault(&mut self, info: PageFaultInfo) -> SysResult<()> {
        if let Some(handler) = self.handler {
            handler(self, info)
        } else {
            panic!("page fault handler: handler not registered");
        }
    }
}

/// Unique data of a specific type of VMA. This enum is used in [`VmArea`].
#[derive(Debug)]
pub enum TypedArea {
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

/// A memory-backed VMA.
///
/// This is a type for debugging purposes. A memory-backed VMA takes a slice of
/// memory as its backing store.
#[derive(Debug)]
pub struct MemoryBackedArea {
    /// The memory backing store.
    pub memory: &'static [u8],
    /// Allocated physical pages.
    pub pages: Vec<FrameTracker>,
}

impl MemoryBackedArea {
    /// Creates a new memory-backed VMA.
    pub fn new(memory: &'static [u8]) -> Self {
        Self {
            memory,
            pages: Vec::new(),
        }
    }

    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        // Extract the specific data of the memory-backed VMA.
        let Self { memory, pages } = match &mut area.map_type {
            TypedArea::MemoryBacked(memory_backed) => memory_backed,
            _ => panic!("fault_handler: not a memory-backed area"),
        };

        let VmArea {
            start_va,
            end_va,
            flags,
            perm,
            ..
        } = *area;

        let PageFaultInfo {
            addr,
            page_table,
            access,
        } = info;

        // Check permission.
        if !access.contains(perm) {
            return Err(SysError::EACCES);
        }

        // Allocate a frame and map the page.
        let frame = FrameTracker::new()?;
        page_table.map_page(addr.page_number(), frame.as_ppn(), flags);
        pages.push(frame);

        // Copy data from the memory backing store to the allocated frame.
        let fault_page = addr.page_number();
        let dst_start = {
            let fault_page_start = fault_page.address().to_usize();
            let start_va = start_va.to_usize();
            usize::max(fault_page_start, start_va)
        };
        let dst_end = {
            let fault_page_end = fault_page.address().to_usize() + PAGE_SIZE;
            let end_va = end_va.to_usize();
            usize::min(fault_page_end, end_va)
        };
        let dst_slice =
            unsafe { slice::from_raw_parts_mut(dst_start as *mut u8, dst_end - dst_start) };
        let src_slice = &memory[(dst_start - start_va.to_usize())..(dst_end - start_va.to_usize())];
        dst_slice.copy_from_slice(src_slice);

        Ok(())
    }
}
