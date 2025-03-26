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

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use arch::riscv64::mm::sfence_vma_addr;
use core::fmt::Debug;
use vfs::page::Page;

use mm::address::{PhysPageNum, VirtAddr, VirtPageNum};
use systype::{SysError, SysResult};

use super::{
    mem_perm::MemPerm,
    page_table::PageTable,
    pte::{PageTableEntry, PteFlags},
};

/// A virtual memory area (VMA).
///
/// A VMA is a contiguous region of virtual memory in an address space that has
/// a common set of attributes, such as permissions and mapping type.
#[derive(Clone)]
pub struct VmArea {
    /// Starting virtual address.
    start: VirtAddr,
    /// Ending virtual address (exclusive).
    end: VirtAddr,
    /// Cache for leaf page table entry flags, which are default when creating
    /// a new leaf entry.
    flags: PteFlags,
    /// Permission.
    perm: MemPerm,
    /// Allocated physical pages.
    pages: BTreeMap<VirtPageNum, Arc<Page>>,
    /// Unique data of a specific type of VMA.
    map_type: TypedArea,
    /// Page fault handler.
    handler: Option<PageFaultHandler>,
}

/// Unique data of a specific type of VMA. This enum is used in [`VmArea`].
#[derive(Debug, Clone)]
pub enum TypedArea {
    /// A helper VMA representing one in the kernel space.
    Kernel(KernelArea),
    /// A memory-backed VMA.
    MemoryBacked(MemoryBackedArea),
    /// An anonymous VMA.
    ///
    /// An anonymous VMA is not backed by any file or memory. A user heap or stack,
    /// or an area created by `mmap` with `MAP_ANONYMOUS` flag, is an anonymous VMA.
    Anonymous(AnonymousArea),
    /// A heap VMA representing a user heap. This is just a special case of an
    /// anonymous area.
    Heap(AnonymousArea),
    /// A file-backed VMA.
    ///
    /// A file-backed VMA is backed by a file. It is used for memory-mapped files.
    FileBacked,
}

/// Page fault handler function type.
///
/// The handler is responsible for handling a “normal” page fault, which is not a COW page fault
/// or a page fault due to TLB not being flushed. The handler is called when the permission is
/// allowed, the fault is not a COW fault, and the page is not already mapped by another thread.
///
/// The [`Page`] parameter is the physical page allocated for the faulting virtual address, which
/// the handler may need to fill with appropriate data.
type PageFaultHandler = fn(&mut VmArea, PageFaultInfo, Page) -> SysResult<()>;

/// Data passed to a page fault handler.
///
/// This struct is used to pass data to a page fault handler registered in a [`VmArea`].
#[derive(Debug)]
pub struct PageFaultInfo<'a> {
    /// Faulting virtual address.
    pub fault_addr: VirtAddr,
    /// Page table.
    pub page_table: &'a mut PageTable,
    /// How the address was accessed when the fault occurred. Only one bit should be set.
    pub access: MemPerm,
}

impl VmArea {
    /// Constructs a global [`VmArea`] whose specific type is [`KernelArea`].
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `flags` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_kernel(start_va: VirtAddr, end_va: VirtAddr, flags: PteFlags) -> Self {
        Self {
            start: start_va,
            end: end_va.round_up(),
            // Set bits A and D because kernel pages are never swapped out.
            flags: flags | PteFlags::V | PteFlags::G | PteFlags::A | PteFlags::D,
            perm: MemPerm::from(flags),
            pages: BTreeMap::new(),
            map_type: TypedArea::Kernel(KernelArea),
            handler: None,
        }
    }

    /// Constructs a user space [`VmArea`] whose specific type is [`MemoryBackedArea`].
    ///
    /// `start_va` is the virtual address from which data in `memory` is mapped, not the
    /// starting virtual address of the VMA.
    ///
    /// `flags` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_memory_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: PteFlags,
        memory: &'static [u8],
    ) -> Self {
        Self {
            start: start_va.round_down(),
            end: end_va.round_up(),
            flags: flags | PteFlags::V | PteFlags::U,
            perm: MemPerm::from(flags),
            pages: BTreeMap::new(),
            map_type: TypedArea::MemoryBacked(MemoryBackedArea::new(memory, start_va)),
            handler: Some(MemoryBackedArea::fault_handler),
        }
    }

    /// Constructs a user space stack area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_stack(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        Self {
            start: start_va,
            end: end_va,
            flags: PteFlags::V | PteFlags::R | PteFlags::W | PteFlags::U,
            perm: MemPerm::R | MemPerm::W | MemPerm::U,
            pages: BTreeMap::new(),
            map_type: TypedArea::Anonymous(AnonymousArea),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Constructs a user space heap area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_heap(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        Self {
            start: start_va,
            end: end_va,
            flags: PteFlags::V | PteFlags::R | PteFlags::W | PteFlags::U,
            perm: MemPerm::R | MemPerm::W | MemPerm::U,
            pages: BTreeMap::new(),
            map_type: TypedArea::Heap(AnonymousArea),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Returns whether this VMA is a heap.
    pub fn is_heap(&self) -> bool {
        matches!(self.map_type, TypedArea::Heap(_))
    }

    /// Handles a page fault happened in this VMA.
    ///
    /// # Errors
    /// Returns [`SysError::EFAULT`] if the access permission is not allowed.
    /// Otherwise, returns [`SysError::ENOMEM`] if a new frame cannot be allocated.
    pub fn handle_page_fault(&mut self, mut info: PageFaultInfo) -> SysResult<()> {
        let &mut VmArea { flags, perm, .. } = self;
        let &mut PageFaultInfo {
            fault_addr,
            ref mut page_table,
            access,
        } = &mut info;

        // Check permission.
        if !perm.contains(access) {
            log::trace!(
                "MemoryBackedArea::fault_handler: access {:?} not allowed, permision is {:?}",
                access,
                perm
            );
            return Err(SysError::EFAULT);
        }

        match page_table.map_page(fault_addr.page_number(), flags)? {
            Ok(page) => {
                self.handler.unwrap()(self, info, page)?;
            }
            Err(pte) => {
                if !pte.flags().contains(PteFlags::W) && access.contains(MemPerm::W) {
                    // Copy-on-write page fault.
                    self.handle_cow_fault(fault_addr, pte)?;
                } else {
                    // If the page is already mapped by another thread, just flush the TLB.
                    sfence_vma_addr(fault_addr.to_usize());
                }
            }
        }
        Ok(())
    }

    /// Handles a COW page fault.
    fn handle_cow_fault(
        &mut self,
        fault_addr: VirtAddr,
        pte: &mut PageTableEntry,
    ) -> SysResult<()> {
        let fault_vpn = fault_addr.page_number();
        let fault_page = self.pages.get(&fault_vpn).unwrap();
        if Arc::strong_count(fault_page) > 1 {
            // Allocate a new page and copy the content if the page is shared.
            let new_page = Page::build()?;
            new_page.copy_from(fault_page);
            let mut new_pte = *pte;
            new_pte.set_flags(new_pte.flags() | PteFlags::W);
            new_pte.set_ppn(new_page.ppn());
            *pte = new_pte;
            sfence_vma_addr(fault_addr.to_usize());
            // Here, the `insert` will drop the old `Arc<Page>` tracked by the VMA,
            // which will decrement the reference count of the `Page`.
            self.pages.insert(fault_vpn, Arc::new(new_page));
        } else {
            // If the page is not shared, just set the write bit.
            let mut new_pte = *pte;
            new_pte.set_flags(new_pte.flags() | PteFlags::W);
            *pte = new_pte;
            sfence_vma_addr(fault_addr.to_usize());
        }
        Ok(())
    }

    pub fn contains(&self, va: VirtAddr) -> bool {
        va >= self.start && va < self.end
    }

    /// Returns the starting virtual address of the VMA.
    pub fn start_va(&self) -> VirtAddr {
        self.start
    }

    /// Sets the starting virtual address of the VMA.
    ///
    /// # Safety
    /// The caller must ensure that the new starting virtual address is page-aligned,
    /// and the new range of the VMA does not overlap with other VMAs.
    pub unsafe fn set_start_va(&mut self, start_va: VirtAddr) {
        self.start = start_va;
    }

    /// Returns the ending virtual address of the VMA.
    pub fn end_va(&self) -> VirtAddr {
        self.end
    }

    /// Sets the ending virtual address of the VMA.
    ///
    /// # Safety
    /// The caller must ensure that the new starting virtual address is page-aligned,
    /// and the new range of the VMA does not overlap with other VMAs.
    pub unsafe fn set_end_va(&mut self, end_va: VirtAddr) {
        self.end = end_va;
    }

    /// Returns the PTE flags of the VMA.
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// Returns the mapping from virtual page numbers to `Arc<Page>`s mapped in this VMA.
    pub fn pages(&self) -> &BTreeMap<VirtPageNum, Arc<Page>> {
        &self.pages
    }
}

impl Debug for VmArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VmArea")
            .field("start_va", &self.start)
            .field("end_va", &self.end)
            .field("flags", &self.flags)
            .field("perm", &self.perm)
            .field("num of pages", &self.pages.len())
            .field("map_type", &self.map_type)
            .finish()
    }
}

/// A helper VMA representing one in the kernel space.
///
/// This struct is used to map an area in the kernel space to the kernel page table.
/// It is of no use after the kernel page table is set up.
///
/// A kernel area must be aligned to the size of a page.
#[derive(Debug, Clone)]
pub struct KernelArea;

impl KernelArea {
    /// Maps the kernel area to the kernel page table.
    pub fn map(area: &VmArea, page_table: &mut PageTable) {
        match area.map_type {
            TypedArea::Kernel(_) => {}
            _ => panic!("KernelArea::map: not a kernel area"),
        }

        let &VmArea {
            start: start_va,
            end: end_va,
            flags,
            ..
        } = area;

        let start_vpn = start_va.page_number();
        let end_vpn = end_va.page_number();
        let start_ppn = start_vpn.to_ppn_kernel().to_usize();
        let end_ppn = end_vpn.to_ppn_kernel().to_usize();
        let ppns = (start_ppn..end_ppn)
            .map(PhysPageNum::new)
            .collect::<Vec<_>>();

        page_table.map_range_to(start_vpn, &ppns, flags).unwrap();
    }
}

/// A memory-backed VMA.
///
/// This is a temporary VMA used as an analogy to a file-backed VMA.
///
/// `memroy` is mapped from `start_va` to `start_va + memory.len()`. The region before
/// `start_va` and after `VmArea::start` is not mapped. The region after
/// `start_va + memory.len()` and before `VmArea::end` is filled with zeros, which is
/// like the `.bss` section in an executable file.
#[derive(Clone)]
pub struct MemoryBackedArea {
    /// The memory backing store.
    memory: &'static [u8],
    /// The virtual address from which `memory` is mapped.
    start_va: VirtAddr,
}

impl MemoryBackedArea {
    /// Creates a new memory-backed VMA.
    fn new(memory: &'static [u8], start_va: VirtAddr) -> Self {
        Self { memory, start_va }
    }

    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo, page: Page) -> SysResult<()> {
        // Extract data needed for fault handling.
        let &mut Self { memory, start_va } = match &mut area.map_type {
            TypedArea::MemoryBacked(memory_backed) => memory_backed,
            _ => panic!("fault_handler: not a memory-backed area"),
        };
        let &mut VmArea {
            end: end_va,
            ref mut pages,
            ..
        } = area;
        let PageFaultInfo { fault_addr, .. } = info;

        // Fill the page with appropriate data.
        // There are 3 types of regions in the page:
        // 1. Region to fill with data from the memory backing store.
        // 2. Region to fill with zeros. (addr >= start_va + memory.len() && addr < end_va)
        // 3. Region that is not in the VMA thus not filled. (addr < start_va || addr >= end_va)
        let page_start = fault_addr.round_down();
        let page_end = VirtAddr::new(fault_addr.to_usize() + 1).round_up();
        let fill_start = VirtAddr::max(start_va, page_start);
        let fill_end = VirtAddr::min(end_va, page_end);
        let fill_len = fill_end.to_usize() - fill_start.to_usize();
        let page_offset = fill_start.page_offset();
        let area_offset = fill_start.to_usize() - start_va.to_usize();
        let back_store_len = memory.len();
        if area_offset < back_store_len {
            // If there is a type 1 region in the page:
            let copy_len = usize::min(back_store_len - area_offset, fill_len);
            let memory_copy_from = &memory[area_offset..area_offset + copy_len];
            let (memory_copy_to, memory_fill_zero) = page
                .bytes_array_range(page_offset..page_offset + fill_len)
                .split_at_mut(copy_len);
            memory_copy_to.copy_from_slice(memory_copy_from);
            memory_fill_zero.fill(0);
            page.bytes_array_range(0..page_offset).fill(0);
        } else {
            // If there is no type 1 region in the page:
            page.bytes_array_range(page_offset..page_offset + fill_len)
                .fill(0);
        }

        pages.insert(fault_addr.page_number(), Arc::new(page));

        Ok(())
    }
}

impl Debug for MemoryBackedArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemoryBackedArea")
            .field(
                "memory back store addr",
                &format_args!("{:p}", self.memory.as_ptr()),
            )
            .field(
                "memory back store len",
                &format_args!("{:#x}", self.memory.len()),
            )
            .finish()
    }
}

/// An anonymous VMA which is not backed by a file or device, such as a user heap or stack.
#[derive(Debug, Clone)]
pub struct AnonymousArea;

impl AnonymousArea {
    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo, page: Page) -> SysResult<()> {
        let &mut VmArea { ref mut pages, .. } = area;
        let PageFaultInfo { fault_addr, .. } = info;

        page.bytes_array().fill(0);
        pages.insert(fault_addr.page_number(), Arc::new(page));

        Ok(())
    }
}
