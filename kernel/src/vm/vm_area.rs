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
use core::{fmt::Debug, mem};

use bitflags::bitflags;

use arch::riscv64::mm::{sfence_vma_addr, sfence_vma_all_except_global};
use config::mm::{KERNEL_MAP_OFFSET, PAGE_SIZE};
use mm::{
    address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum},
    page_cache::page::Page,
};
use systype::{SysError, SysResult};
use vfs::file::File;

use super::{
    mem_perm::MemPerm,
    page_table::PageTable,
    pte::{PageTableEntry, PteFlags},
};

/// A virtual memory area (VMA).
///
/// A VMA is a contiguous, page-aligned region of virtual memory in an address
/// space that has a common set of attributes, such as permissions and mapping type.
#[derive(Clone)]
pub struct VmArea {
    /// Starting virtual address, page-aligned.
    start: VirtAddr,
    /// Ending virtual address (exclusive), page-aligned.
    end: VirtAddr,
    /// Flags of the VMA.
    flags: VmaFlags,
    /// Memory protection of the VMA.
    prot: MemPerm,
    /// Cache for leaf page table entry flags, which are default when creating
    /// a new leaf entry.
    pte_flags: PteFlags,
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
/// The handler is responsible for handling a “normal” page fault, which is not a CoW page fault
/// or a page fault due to TLB not being flushed. The handler is called when the permission is
/// allowed, the fault is not a CoW fault, and the page is not already mapped by another thread.
type PageFaultHandler = fn(&mut VmArea, PageFaultInfo) -> SysResult<()>;

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

bitflags! {
    /// Flags of a VMA.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct VmaFlags: u8 {
        /// The VMA is shared.
        const SHARED = 1 << 0;
        /// The VMA is private.
        const PRIVATE = 1 << 1;
    }
}

impl VmArea {
    /// Constructs a global [`VmArea`] whose specific type is [`OffsetArea`], representing
    /// an area in the kernel space, which has an offset of `KERNEL_MAP_OFFSET` from the
    /// physical address.
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `pte_flags` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_kernel(start_va: VirtAddr, end_va: VirtAddr, pte_flags: PteFlags) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        Self::new_fixed_offset(
            start_va,
            end_va,
            // This field is insignificant for kernel VMAs.
            VmaFlags::PRIVATE,
            // Set bits A and D because kernel pages are never swapped out.
            pte_flags | PteFlags::A | PteFlags::D,
            KERNEL_MAP_OFFSET,
        )
    }

    /// Constructs a global [`VmArea`] whose specific type is [`OffsetArea`], representing
    /// an area in the kernel space. This function is used to map a MMIO region.
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `pte_flags` needs to have `RWXAD` bits set properly; other bits must be zero.
    pub fn new_fixed_offset(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        pte_flags: PteFlags,
        offset: usize,
    ) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        Self {
            start: start_va,
            end: end_va.round_up(),
            flags,
            pte_flags: pte_flags | PteFlags::V | PteFlags::G,
            prot: MemPerm::from(pte_flags),
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
    /// `pte_flags` needs to have `RWX` bits set properly; other bits must be zero.
    #[deprecated]
    pub fn new_memory_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        pte_flags: PteFlags,
        memory: &'static [u8],
    ) -> Self {
        Self {
            start: start_va.round_down(),
            end: end_va.round_up(),
            flags,
            pte_flags: pte_flags | PteFlags::V | PteFlags::U,
            prot: MemPerm::from(pte_flags),
            pages: BTreeMap::new(),
            map_type: TypedArea::MemoryBacked(MemoryBackedArea::new(memory, start_va)),
            handler: Some(MemoryBackedArea::fault_handler),
        }
    }

    /// Constructs a user space [`VmArea`] whose specific type is [`FileBackedArea`].
    ///
    /// `offset` and `len` define the file region to be mapped. `start_va` and `end_va`
    /// define the virtual address range to be mapped to the file region (and possibly
    /// a zero-filling region at the end). If `end_va - start_va` is larger than `len`,
    /// the tailing part of the VMA is filled with zeros.
    ///
    /// To `mmap` a file region, `offset` and `len` must be page-aligned, and `len` must
    /// equal to `end_va - start_va`. If the file region spans to the end of the file,
    /// `len` must be rounded up to the size of a page.
    ///
    /// To load a segment from an executable file, `offset` and `len` do not need to be
    /// page-aligned, and `len` can be smaller than `end_va - start_va`. If `len` is
    /// smaller than that, the VMA must be private, and the tailing part of the VMA is
    /// filled with zeros. Make sure that the file region defined by `offset` and `len`
    /// is within the file.
    ///
    /// `pte_flags` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_file_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        pte_flags: PteFlags,
        file: Arc<dyn File>,
        offset: usize,
        len: usize,
    ) -> Self {
        debug_assert!(
            (len == end_va.to_usize() - start_va.to_usize())
                || ((len < end_va.to_usize() - start_va.to_usize())
                    && flags.contains(VmaFlags::PRIVATE))
        );
        Self {
            start: start_va.round_down(),
            end: end_va.round_up(),
            flags,
            pte_flags: pte_flags | PteFlags::V | PteFlags::U,
            prot: MemPerm::from(pte_flags),
            pages: BTreeMap::new(),
            map_type: TypedArea::FileBacked(FileBackedArea::new(file, offset, len)),
            handler: Some(FileBackedArea::fault_handler),
        }
    }

    /// Constructs a user space stack area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_stack(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);
        Self {
            start: start_va,
            end: end_va,
            flags: VmaFlags::PRIVATE,
            pte_flags: PteFlags::V | PteFlags::R | PteFlags::W | PteFlags::U,
            prot: MemPerm::R | MemPerm::W | MemPerm::U,
            pages: BTreeMap::new(),
            map_type: TypedArea::Anonymous(AnonymousArea),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Constructs a user space heap area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_heap(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);
        Self {
            start: start_va,
            end: end_va,
            flags: VmaFlags::PRIVATE,
            pte_flags: PteFlags::V | PteFlags::R | PteFlags::W | PteFlags::U,
            prot: MemPerm::R | MemPerm::W | MemPerm::U,
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
    /// the range covers the starting part or ending part of the original VMA,
    /// so only a single VMA is left. If both are `None`, it means the range covers
    /// the whole VMA, so the original VMA is totally removed.
    pub fn unmap_range(
        mut self,
        page_table: &mut PageTable,
        remove_from: VirtAddr,
        remove_to: VirtAddr,
    ) -> (Option<Self>, Option<Self>) {
        debug_assert!(remove_from < remove_to);
        let remove_from = VirtAddr::max(self.start, remove_from);
        let remove_to = VirtAddr::min(self.end, remove_to);
        if remove_from == self.start && remove_to == self.end {
            // The range to be removed is the whole VMA.
            return (None, None);
        }
        if remove_from >= remove_to {
            // The range to be removed is empty.
            return (Some(self), None);
        }

        // Re-assign `Page`s in the old VMA to the new VMA(s).
        let (pages_low, pages_mid, pages_high) = {
            let mut pages = mem::take(&mut self.pages);
            let mut pages_mid_high = pages.split_off(&remove_from.page_number());
            let pages_high = pages_mid_high.split_off(&remove_to.page_number());
            (pages, pages_mid_high, pages_high)
        };

        // Invalidate the page table entries in the range to be removed.
        for (vpn, _) in pages_mid {
            let pte = page_table.find_entry(vpn).unwrap();
            *pte = PageTableEntry::default();
        }

        let vma_low = if remove_from > self.start {
            let mut vma = self.clone();
            vma.end = remove_from;
            vma.pages = pages_low;
            // Note: we may need to extract the updating logic of fields in specific
            // `TypedArea` structs to a common function, and 
            if let TypedArea::FileBacked(file_backed) = &mut vma.map_type {
                file_backed.len = file_backed
                    .len
                    .min(remove_from.to_usize() - vma.start.to_usize());
            }
            Some(vma)
        } else {
            None
        };
        let vma_high = if remove_to < self.end {
            let mut vma = self.clone();
            vma.start = remove_to;
            vma.pages = pages_high;
            if let TypedArea::FileBacked(file_backed) = &mut vma.map_type {
                file_backed.len = file_backed
                    .len
                    .saturating_sub(remove_to.to_usize() - vma.start.to_usize());
                file_backed.offset += remove_to.to_usize() - vma.start.to_usize();
            }
            Some(vma)
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
        let &mut VmArea {
            pte_flags, prot, ..
        } = self;
        let PageFaultInfo {
            fault_addr,
            ref mut page_table,
            access,
        } = info;

        // Check the protection bits.
        if !prot.contains(access) {
            log::warn!(
                "VmArea::handle_page_fault: access {:?} at {:#x} not allowed, with protection {:?}",
                access,
                fault_addr.to_usize(),
                prot,
            );
            return Err(SysError::EFAULT);
        }

        let pte = {
            let (pte, flush_all) =
                page_table.find_entry_force(fault_addr.page_number(), pte_flags)?;
            if flush_all {
                sfence_vma_all_except_global();
            }
            pte
        };
        if pte.is_valid() {
            if access == MemPerm::W && !pte.flags().contains(PteFlags::W) {
                // Copy-on-write page fault.
                self.handle_cow_fault(fault_addr, pte)?;
            } else {
                // The page is already mapped by another thread, so just flush the TLB.
                sfence_vma_addr(fault_addr.to_usize());
            }
        } else {
            self.handler.unwrap()(self, info)?;
        }

        Ok(())
    }

    /// Handles a copy-on-write page fault.
    ///
    /// This function is called when a page fault occurs due to a write access to a
    /// copy-on-write page. The function allocates a new page and copies the content
    /// from the original page to the new page. The new page is then mapped to the
    /// faulting virtual address.
    fn handle_cow_fault(
        &mut self,
        fault_addr: VirtAddr,
        pte: &mut PageTableEntry,
    ) -> SysResult<()> {
        let fault_vpn = fault_addr.page_number();
        let fault_page = self.pages.get(&fault_vpn).unwrap();
        // Note: The if-else branch does not work as expected because the reference
        // count of a page is not necessarily 1 when the page is not shared. For
        // example, the page may be in the page cache of a file, which increments the
        // reference count of the page. The current implementation is not buggy, but
        // it is not optimal.
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
        self.pte_flags
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
            .field("pte_flags", &self.pte_flags)
            .field("prot", &self.prot)
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
            pte_flags,
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

        page_table
            .map_range_to(start_vpn, &ppns, pte_flags)
            .unwrap();
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
#[deprecated]
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
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let &mut Self { memory, start_va } = match &mut area.map_type {
            TypedArea::MemoryBacked(memory_backed) => memory_backed,
            _ => panic!("fault_handler: not a memory-backed area"),
        };
        let &mut VmArea {
            end: end_va,
            ref mut pages,
            ..
        } = area;
        let PageFaultInfo {
            fault_addr,
            page_table,
            ..
        } = info;

        let page = Page::build()?;
        page_table.map_page_to(fault_addr.page_number(), page.ppn(), area.pte_flags)?;

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
/// This struct is both used to `mmap` a file-backed area to a user process, and to
/// load a loadable segment from an executable file.
///
/// A VMA of this kind is backed by a file. It maps a region in a file, or a file region,
/// to an area in the virtual address space of a process. A file region is defined as a
/// contiguous region of a file, which is not necessarily page-aligned, and specially,
/// a file region may span beyond the end of the file, at most to the end of the last
/// page of the file. (Note that the last page of a file may or may not be a page full of
/// file data, because the file size is not necessarily a multiple of page size.) The
/// tailing part of the page is filled with zeros.
///
/// This struct defines the range of a file region to a VMA by its `offset` and `len`
/// fields. The `offset` field is a page-aligned offset in the file, pointing to the
/// start of the file region. The `len` field is the length of the file region, which is
/// not necessarily page-aligned, and can span at most to the end of the last page of the
/// file. The VMA itself is page-aligned, and can be larger than the file region at its
/// end. The region from the end of the file region to the end of the VMA is filled with
/// zeros.
///
/// There is a special case of a file region: 0-length file region. A VMA that has a
/// 0-length file region is completely filled with zeros. Specially, a such VMA allows an
/// `offset` that is beyond the last page of the file, which would violates the
/// definition of a file region if its length was not 0.
///
/// ## Unification of `mmap`ing and loading an executable file segment
///
/// There are two origins of file-backed VMAs: `mmap/munmap` and loading an executable
/// file segment.
///
/// `mmap` always maps a file by pages. That is, it always tries to map a file region
/// whose starting offset and ending offset is page-aligned. If there is a partial page
/// at the end of the file, the tailing part is filled with zeros. In addition, the
/// length of the VMA is the same as the length of the file region (`len`). This is
/// perfectly corresponds to the design of page cache, which caches a file by pages
/// as well. Only a point here: to satisfy the zero-filling requirement, the page cache
/// is required to fill the tailing part of the last partial page with zeros, which is
/// resonable.
///
/// `munmap` may split a VMA or change the size of a VMA, so new VMA structs are created.
/// However, because `munmap` also unmaps by pages, the unmapping is very simple to
/// implement, so we do not write more about it here.
///
/// Loading an executable file is largely different from `mmap/munmap`. The VMA itself
/// must still be page-aligned, but the file region is not necessarily so. For example,
/// a file region starts from 0x1789 and ends at 0x3456, and the `len` field is 0x4100.
/// The VMA will start from CONST + 0x1000 (rounded down from 0x1789) and end at
/// CONST + 0x6000 (rounded up from 0x1789 + 0x4100). The region from 0x1000 to 0x1789
/// is filled with whatever data from file offset 0x1000 to 0x1789. The region from
/// 0x1789 to 0x3456 is the data we really want (e.g., a .data segment). The region from
/// 0x3456 to 0x5889 (0x1789 + 0x4100) is filled with zeros as a .bss segment. The region
/// from 0x5789 to 0x6000 is filled zeros as well, but they are not part of the .bss
/// segment.
///
/// Note that data from offset 0x1000 to 0x1789 will not cause problems if the ELF file
/// is well-formed. For example, an executable segment may never share a page with a
/// writable segment, so the data in the writable segment will not be executed by mistake.
/// On the other hand, a read-only segment may share a page with a read/write segment,
/// because 1) read data from the read/write segment as if it is a read-only segment does
/// not cause problems, and 2) when the read/write segment is written to, a copy-on-write
/// page fault occurs, so the write will not corrupt the read-only segment.
///
/// Therefore, we can just re-define a file region: it starts from a page-aligned offset,
/// and may or may not end at a page-aligned offset. Formerly mentioned starting offset
/// 0x1789 can be simply transformed to 0x1000. However, to simplify the use of this
/// struct, we will do the transformation in the constructor of this struct, so that the
/// caller need not worry about the details.
///
/// The definition of a file region brings a great convenience to unifying `mmap`ing and
/// loading an executable file segment as one VMA struct—the former is a special case of
/// the latter, requiring the end of a file region to be page-aligned, and that the
/// length of the VMA is the same as the length of the file region. Therefore, we can
/// just use a single algorithm to handle mapping, unmapping, and page fault handling.
///
/// ## Protection, sharing, and page cache
///
/// There are some different types of file-backed VMAs based on `prot` and `flags`
/// fields, and the differences in their implementations are noted below.
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
///   the page cache of the file, as if it is a private, read-only VMA. This violates
///   the semantics of private, read-only VMAs, but it is OK; see the documentation of
///   `MAP_PRIVATE` flag in `mmap(2)`:
///   > It is unspecified whether changes made to the file after the mmap() call are
///   > visible in the mapped region.
///
/// The area from the end of the file region to the end of the VMA has no corresponding
/// file region, so a VMA that has such a region must be private.
#[derive(Clone)]
pub struct FileBackedArea {
    /// The file backing store.
    file: Arc<dyn File>,
    /// The page-aligned offset of the mapped region in the file.
    offset: usize,
    /// The length of the mapped region in the file.
    len: usize,
}

impl FileBackedArea {
    /// Creates a new file-backed VMA.
    ///
    /// `file` is the file backing store.
    /// `offset` is the starting offset in the file.
    /// `len` is the length of the mapped region in the file.
    ///
    /// If the user wants to `mmap` a file region, `offset` and `len` must be
    /// page-aligned. If the file region spans to the end of the file, `len` must be
    /// rounded up to the size of a page.
    ///
    /// If the user wants to load a segment from an executable file, `offset` and
    /// `len` should just be the starting offset and length of the segment; the
    /// user need not worry about alignment.
    pub fn new(file: Arc<dyn File>, offset: usize, len: usize) -> Self {
        let offset_aligned = offset / PAGE_SIZE * PAGE_SIZE;
        let len_aligned = len + offset % PAGE_SIZE;
        Self {
            file,
            offset: offset_aligned,
            len: len_aligned,
        }
    }

    /// Handles a page fault.
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let &FileBackedArea {
            ref file,
            offset,
            len: region_len,
        } = match &area.map_type {
            TypedArea::FileBacked(file_backed) => file_backed,
            _ => panic!("FileBackedArea::fault_handler: not a file-backed area"),
        };
        let &mut VmArea {
            start: start_va,
            flags,
            mut pte_flags,
            ..
        } = area;
        let PageFaultInfo {
            fault_addr,
            page_table,
            access,
        } = info;

        // Offset from the start of the VMA to the faulting page.
        let area_offset = fault_addr.round_down().to_usize() - start_va.to_usize();
        if area_offset >= region_len {
            // The whole page is after the file region, so allocate a zeroed page for it.
            let page = Page::build()?;
            page.as_mut_slice().fill(0);
            page_table.map_page_to(fault_addr.page_number(), page.ppn(), pte_flags)?;
            area.pages.insert(fault_addr.page_number(), Arc::new(page));
            return Ok(());
        }

        // Offset from the start of the file to the start of the page to be mapped.
        let file_offset = offset + area_offset;
        let cached_page = match file.inode().page_cache().get_page(file_offset) {
            Some(page) => page,
            None => {
                // The page is not in the page cache, so we need to read it from the file
                // and insert it into the page cache.
                // Note: Consider extracting this to a function of `PageCache` or `Inode`.
                let page = Arc::new(Page::build()?);
                file.base_read(page.as_mut_slice(), file_offset)?;
                file.inode()
                    .page_cache()
                    .insert_page(file_offset, Arc::clone(&page));
                page
            }
        };

        if area_offset + PAGE_SIZE > region_len {
            // Part of the page is in the file region, and part of the page is not.
            // Similar to the case where the whole page is not in the file region,
            // we allocate a new page and copy the first part of the page from the
            // file region, and fill the rest with zeros.
            let page = Page::build()?;
            let copy_len = region_len - area_offset;
            let page_copy_from = &cached_page.as_slice()[..copy_len];
            let (page_copy_to, page_fill_zero) = page.as_mut_slice().split_at_mut(copy_len);
            page_copy_to.copy_from_slice(page_copy_from);
            page_fill_zero.fill(0);
            page_table.map_page_to(fault_addr.page_number(), page.ppn(), pte_flags)?;
            area.pages.insert(fault_addr.page_number(), Arc::new(page));
            return Ok(());
        }

        // The page to be mapped to the faulting address.
        let page = if flags.contains(VmaFlags::PRIVATE) && access == MemPerm::W {
            // Write to a private VMA: Do copy-on-write beforehand.
            let page = Page::build()?;
            page.copy_from_page(&cached_page);
            Arc::new(page)
        } else {
            // Other conditions: Just use the cached page.
            if flags.contains(VmaFlags::PRIVATE) {
                // Read from or execute a private VMA: Mark the page as read-only, which
                // posiibly means copy-on-write.
                pte_flags = pte_flags.difference(PteFlags::W);
            }
            cached_page
        };
        page_table.map_page_to(fault_addr.page_number(), page.ppn(), pte_flags)?;
        area.pages.insert(fault_addr.page_number(), page);
        Ok(())
    }
}

impl Debug for FileBackedArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileBackedArea")
            .field("file", &self.file.dentry().name())
            .field("offset", &self.offset)
            .finish()
    }
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
    pub fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let &mut VmArea {
            ref mut pages,
            flags,
            pte_flags,
            ..
        } = area;
        let PageFaultInfo {
            fault_addr,
            page_table,
            ..
        } = info;

        if flags.contains(VmaFlags::SHARED) {
            unimplemented!("Handling a page fault in a shared anonymous VMA");
        }

        let page = Page::build()?;
        page_table.map_page_to(fault_addr.page_number(), page.ppn(), pte_flags)?;
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
