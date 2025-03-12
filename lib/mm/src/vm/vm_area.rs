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

use alloc::{slice, vec::Vec};
use config::mm::PAGE_SIZE;
use core::fmt::Debug;

use bitflags::bitflags;

use super::pte::PteFlags;
use crate::{address::VirtAddr, frame::FrameTracker};

bitflags! {
    /// Mapping permission corresponding to R, W, X, and U bits in a page table entry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapPerm: u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const X = 1 << 2;
        const U = 1 << 3;
    }
}

impl From<MapPerm> for PteFlags {
    /// Convert `MapPerm` to `PteFlags`.
    fn from(perm: MapPerm) -> Self {
        let mut flags = Self::empty();
        if perm.contains(MapPerm::U) {
            flags |= PteFlags::U;
        } else {
            flags |= PteFlags::G;
        }
        if perm.contains(MapPerm::R) {
            flags |= PteFlags::R;
        }
        if perm.contains(MapPerm::W) {
            flags |= PteFlags::W;
        }
        if perm.contains(MapPerm::X) {
            flags |= PteFlags::X;
        }
        flags
    }
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
    perm: MapPerm,
    /// Unique data of a specific type of VMA.
    map_type: TypedArea,
    /// Page fault handler.
    handler: Option<fn(&Self, VirtAddr)>,
}

impl VmArea {
    /// Constructs a [`VmArea`] which is a [`TypedArea::MemoryBacked`] VMA.
    pub fn new_memory_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: PteFlags,
        perm: MapPerm,
        memory: &'static [u8],
    ) -> Self {
        Self {
            start_va,
            end_va,
            flags,
            perm,
            map_type: TypedArea::MemoryBacked(MemoryBackedArea::new(memory)),
            handler: Some(Self::memory_backed_fault_handler),
        }
    }

    /// Returns the starting virtual page number of the VMA.
    pub fn start_va(&self) -> VirtAddr {
        self.start_va
    }

    /// Returns the ending virtual page number of the VMA.
    pub fn end_va(&self) -> VirtAddr {
        self.end_va
    }

    /// Returns the page table entry flags of the VMA.
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// Returns the permission of the VMA.
    pub fn perm(&self) -> MapPerm {
        self.perm
    }

    /// Page fault handler for a [`MemoryBackedArea`].
    fn memory_backed_fault_handler(&self, fault_addr: VirtAddr) {
        if let TypedArea::MemoryBacked(memory_backed) = &self.map_type {
            memory_backed.fault_handler(fault_addr, self.start_va, self.end_va);
        } else {
            unreachable!("Page fault handler: not a MemoryBackedArea");
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
    pub fn fault_handler(&self, fault_addr: VirtAddr, start_va: VirtAddr, end_va: VirtAddr) {
        let fault_page = fault_addr.page_number();
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

        let src_slice =
            &self.memory[(dst_start - start_va.to_usize())..(dst_end - start_va.to_usize())];

        dst_slice.copy_from_slice(src_slice);
    }
}
