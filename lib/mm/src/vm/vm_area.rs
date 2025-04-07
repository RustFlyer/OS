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
use config::mm::KERNEL_MAP_OFFSET;
use core::{fmt::Debug, mem};

use arch::riscv64::mm::sfence_vma_addr;

use systype::{SysError, SysResult};

use super::{
    mem_perm::MemPerm,
    page_cache::page::Page,
    page_table::PageTable,
    pte::{PageTableEntry, PteFlags},
};
use crate::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};

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
    /// A fixed-offset VMA.
    ///
    /// A fixed-offset VMA is used to map physical addresses to virtual addresses
    /// by adding a fixed offset. This is used for kernel space and MMIO regions.
    /// This kind of VMAs do not have a page fault handler, and a page fault will
    /// never occur in this kind of VMAs.
    Offset(OffsetArea),
    /// A memory-backed VMA.
    ///
    /// A memory-backed VMA is backed by a memory region. This is a temporary VMA
    /// used as an analogy to a file-backed VMA.
    MemoryBacked(MemoryBackedArea),
    /// A file-backed VMA.
    ///
    /// A file-backed VMA is backed by a file. It is created when loading an executable
    /// file or `mmap`ing a file.
    FileBacked(FileBackedArea),
    /// An anonymous VMA.
    ///
    /// An anonymous VMA is not backed by any file or memory. A user stack or an area
    /// created by `mmap` with `MAP_ANONYMOUS` flag, is an anonymous VMA.
    Anonymous(AnonymousArea),
    /// A heap VMA representing a user heap. This is just a special case of an
    /// anonymous area.
    Heap(AnonymousArea),
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
    /// Constructs a global [`VmArea`] whose specific type is [`OffsetArea`], representing
    /// an area in the kernel space, which has an offset of `KERNEL_MAP_OFFSET` from the
    /// physical address.
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `flags` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_kernel(start_va: VirtAddr, end_va: VirtAddr, flags: PteFlags) -> Self {
        Self::new_fixed_offset(
            start_va,
            end_va,
            // Set bits A and D because kernel pages are never swapped out.
            flags | PteFlags::A | PteFlags::D,
            KERNEL_MAP_OFFSET,
        )
    }

    /// Constructs a global [`VmArea`] whose specific type is [`OffsetArea`], representing
    /// an area in the kernel space. This function is used to map a MMIO region.
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `flags` needs to have `RWXAD` bits set properly; other bits must be zero.
    pub fn new_fixed_offset(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: PteFlags,
        offset: usize,
    ) -> Self {
        Self {
            start: start_va,
            end: end_va.round_up(),
            flags: flags | PteFlags::V | PteFlags::G,
            perm: MemPerm::from(flags),
            pages: BTreeMap::new(),
            map_type: TypedArea::Offset(OffsetArea { offset }),
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

    /// Removes mappings in a given range of virtual addresses from the VMA
    /// in a given [`PageTable`], possibly splitting the VMA and invalidating
    /// page table entries.
    ///
    /// The range is defined by `remove_from` and `remove_to`, which must both be
    /// page-aligned. The range is inclusive of `remove_from` and exclusive of
    /// `remove_to`. `remove_from` and `remove_to` do not need to be contained
    /// in the VMA; only range that overlaps with the VMA is removed. It is allowed
    /// that the range does not overlap with the VMA at all, in which case the VMA is
    /// unchanged.
    ///
    /// `remove_from` must be less than `remove_to`.
    ///
    /// Returns a tuple of two `Option<VmArea>`, which are the new VMAs created
    /// by splitting the original VMA. If one of the new VMAs is `None`, it means
    /// the range covers the starting or ending part of the original VMA, which
    /// leaves only a single VMA. If both are `None`, it means the range covers
    /// the whole VMA, so the original VMA is totally removed.
    ///
    /// # Note
    /// This function is buggy because it does not update fields in specific
    /// [`TypedArea`] structs in the [`VmArea`] struct.
    pub fn unmap_range(
        mut self,
        page_table: &mut PageTable,
        remove_from: VirtAddr,
        remove_to: VirtAddr,
    ) -> (Option<Self>, Option<Self>) {
        debug_assert!(remove_from < remove_to);
        let start_va = VirtAddr::max(self.start, remove_from);
        let end_va = VirtAddr::min(self.end, remove_to);

        if start_va == self.start && end_va == self.end {
            // The range to be removed is the whole VMA.
            return (None, None);
        }
        if start_va >= end_va {
            // The range to be removed is empty.
            return (Some(self), None);
        }

        // Re-assign `Page`s in the old VMA to the new VMA(s).
        let (pages_low, pages_mid, pages_high) = {
            let mut pages = mem::take(&mut self.pages);
            let mut pages_mid_high = pages.split_off(&start_va.page_number());
            let pages_high = pages_mid_high.split_off(&end_va.page_number());
            (pages, pages_mid_high, pages_high)
        };

        // Unmap the pages to be removed in the page table.
        for (vpn, _) in pages_mid {
            let pte = page_table.find_entry(vpn).unwrap();
            *pte = PageTableEntry::default();
        }

        let vma_low = if remove_from > self.start {
            Some(Self {
                end: start_va,
                pages: pages_low,
                ..self.clone()
            })
        } else {
            None
        };
        let vma_high = if remove_to < self.end {
            Some(Self {
                start: end_va,
                pages: pages_high,
                ..self.clone()
            })
        } else {
            None
        };

        (vma_low, vma_high)
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
            new_page.copy_from_page(fault_page);
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

    /// Returns whether this VMA is a heap.
    pub fn is_heap(&self) -> bool {
        matches!(self.map_type, TypedArea::Heap(_))
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

/// A fixed-offset VMA.
///
/// This struct is used to map an area in the kernel space to the kernel page table.
/// It is of no use after the kernel page table is set up.
///
/// An `OffsetArea` must be aligned to the size of a page.
#[derive(Debug, Clone)]
pub struct OffsetArea {
    /// The offset between the physical address and the virtual address of the area.
    /// That is, PA + `offset` = VA.
    offset: usize,
}

impl OffsetArea {
    /// Maps the area to the page table.
    pub fn map(area: &VmArea, page_table: &mut PageTable) {
        let offset = match area.map_type {
            TypedArea::Offset(OffsetArea { offset }) => offset,
            _ => panic!("OffsetArea::map: not a fixed-offset area"),
        };

        let &VmArea {
            start: start_va,
            end: end_va,
            flags,
            ..
        } = area;

        let start_vpn = start_va.page_number();
        let start_ppn = PhysAddr::new(start_va.to_usize() - offset)
            .page_number()
            .to_usize();
        let end_ppn = PhysAddr::new(end_va.to_usize() - offset)
            .page_number()
            .to_usize();
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
            let (memory_copy_to, memory_fill_zero) =
                page.as_mut_slice()[page_offset..page_offset + fill_len].split_at_mut(copy_len);
            memory_copy_to.copy_from_slice(memory_copy_from);
            memory_fill_zero.fill(0);
            page.as_mut_slice()[0..page_offset].fill(0);
        } else {
            // If there is no type 1 region in the frame:
            page.as_mut_slice()[page_offset..page_offset + fill_len].fill(0);
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

/// A file-backed VMA.
///
/// This kind of VMA is backed by a file, i.e., when a page in this kind of VMA is
/// accessed for the first time, the page is read from the file.
///
/// There are some different types of file-backed VMAs based on the permissions, and
/// the differences in their implementations are noted below.
/// - Private, read-only: A page in this kind of VMA is mapped to a page in the page
///   cache of the file. Any write to the same page in the file in another private,
///   writable VMA does not affect the data on this page.
/// - Private, writable: A page in this kind of VMA may or may not be mapped to a page
///   in the page cache of the file, depending on whether the page is written to for
///   the first time. Assuming the page is first read from and then written to, the
///   behavior is as follows:
///   - When the page is first read from, it is mapped to a page in the page cache of
///     the file, and is marked as copy-on-write.
///   - When the page is first written to, a copy-on-write page fault occurs, and a new
///     page is allocated exclusively for this VMA. The data in the original page is
///     copied to the new page.
/// - Shared, read-only or writable: A page in this kind of VMA is mapped to a page in
///   the page cache of the file, as if it is a private, read-only VMA. This is OK;
///   see the documentation of `MAP_PRIVATE` flag in `mmap(2)`:
///   > It is unspecified whether changes made to the file after the mmap() call are
///   > visible in the mapped region.
pub struct FileBackedArea {
    /// The file backing store.
    file: Arc<dyn File>,
    /// The offset in the file from which the VMA is mapped.
    ///
    /// It should be page-aligned.
    offset: usize,
}

/// An anonymous VMA which is not backed by a file or device, such as a user heap or stack.
///
/// Each anonymous VMA is filled with zeros when a process first accesses it, in order to
/// avoid leaking data from other processes or the kernel. This is similar to the
/// `.bss` section in an executable file.
#[derive(Debug, Clone)]
pub struct AnonymousArea;

impl AnonymousArea {
    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo, page: Page) -> SysResult<()> {
        let &mut VmArea { ref mut pages, .. } = area;
        let PageFaultInfo { fault_addr, .. } = info;

        page.as_mut_slice().fill(0);
        pages.insert(fault_addr.page_number(), Arc::new(page));

        Ok(())
    }
}

pub fn test_unmap_range() {
    {
        let mut vma = VmArea::new_kernel(VirtAddr::new(0x1000), VirtAddr::new(0x8000), PteFlags::V);
        let mut page_table = PageTable::build().unwrap();

        for vpn in vma.start_va().page_number().to_usize()..vma.end_va().page_number().to_usize() {
            let vpn = VirtPageNum::new(vpn);
            let page = page_table.map_page(vpn, PteFlags::V).unwrap().unwrap();
            vma.pages.insert(vpn, Arc::new(page));
        }

        let (vma_low, vma_high) = vma.unmap_range(
            &mut page_table,
            VirtAddr::new(0x1000),
            VirtAddr::new(0x5000),
        );

        assert!(vma_low.is_none());
        assert!(vma_high.is_some());
        let vma_high = vma_high.unwrap();
        assert!(vma_high.start_va() == VirtAddr::new(0x5000));
        assert!(vma_high.end_va() == VirtAddr::new(0x8000));
        assert!(vma_high.contains(VirtAddr::new(0x7000)));
        assert!(vma_high.contains(VirtAddr::new(0x5000)));
        assert!(!vma_high.contains(VirtAddr::new(0x4000)));
        assert!(!vma_high.contains(VirtAddr::new(0x1000)));
    }
    {
        let mut vma = VmArea::new_kernel(VirtAddr::new(0x1000), VirtAddr::new(0x8000), PteFlags::V);
        let mut page_table = PageTable::build().unwrap();

        for vpn in vma.start_va().page_number().to_usize()..vma.end_va().page_number().to_usize() {
            let vpn = VirtPageNum::new(vpn);
            let page = page_table.map_page(vpn, PteFlags::V).unwrap().unwrap();
            vma.pages.insert(vpn, Arc::new(page));
        }

        let (vma_low, vma_high) = vma.unmap_range(
            &mut page_table,
            VirtAddr::new(0x3000),
            VirtAddr::new(0x6000),
        );

        assert!(vma_low.is_some());
        let vma_low = vma_low.unwrap();
        assert!(vma_low.start_va() == VirtAddr::new(0x1000));
        assert!(vma_low.end_va() == VirtAddr::new(0x3000));
        assert!(vma_low.contains(VirtAddr::new(0x1000)));
        assert!(vma_low.contains(VirtAddr::new(0x2000)));
        assert!(!vma_low.contains(VirtAddr::new(0x3000)));
        assert!(!vma_low.contains(VirtAddr::new(0x6000)));
        assert!(!vma_low.contains(VirtAddr::new(0x7000)));
        assert!(vma_high.is_some());
        let vma_high = vma_high.unwrap();
        assert!(vma_high.start_va() == VirtAddr::new(0x6000));
        assert!(vma_high.end_va() == VirtAddr::new(0x8000));
        assert!(vma_high.contains(VirtAddr::new(0x7000)));
        assert!(vma_high.contains(VirtAddr::new(0x6000)));
        assert!(!vma_high.contains(VirtAddr::new(0x5000)));
        assert!(!vma_high.contains(VirtAddr::new(0x1000)));
    }
    {
        let mut vma = VmArea::new_kernel(VirtAddr::new(0x1000), VirtAddr::new(0x8000), PteFlags::V);
        let mut page_table = PageTable::build().unwrap();

        for vpn in vma.start_va().page_number().to_usize()..vma.end_va().page_number().to_usize() {
            let vpn = VirtPageNum::new(vpn);
            let page = page_table.map_page(vpn, PteFlags::V).unwrap().unwrap();
            vma.pages.insert(vpn, Arc::new(page));
        }

        let (vma_low, vma_high) = vma.unmap_range(
            &mut page_table,
            VirtAddr::new(0x0000),
            VirtAddr::new(0x9000),
        );

        assert!(vma_low.is_none());
        assert!(vma_high.is_none());
    }
    {
        let mut vma = VmArea::new_kernel(VirtAddr::new(0x5000), VirtAddr::new(0x8000), PteFlags::V);
        let mut page_table = PageTable::build().unwrap();

        for vpn in vma.start_va().page_number().to_usize()..vma.end_va().page_number().to_usize() {
            let vpn = VirtPageNum::new(vpn);
            let page = page_table.map_page(vpn, PteFlags::V).unwrap().unwrap();
            vma.pages.insert(vpn, Arc::new(page));
        }

        let (vma_low, vma_high) = vma.unmap_range(
            &mut page_table,
            VirtAddr::new(0x1000),
            VirtAddr::new(0x4000),
        );

        assert!(vma_low.is_some());
        let vma_low = vma_low.unwrap();
        assert!(vma_low.start_va() == VirtAddr::new(0x5000));
        assert!(vma_low.end_va() == VirtAddr::new(0x8000));
        assert!(vma_low.contains(VirtAddr::new(0x5000)));
        assert!(vma_low.contains(VirtAddr::new(0x7000)));
        assert!(vma_high.is_none());
    }
}
