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
use core::{cmp, fmt::Debug, mem};
use osfs::special::memfd::flags::MemfdSeals;

use bitflags::bitflags;

use arch::{
    mm::{tlb_flush_addr, tlb_shootdown},
    pte::{PageTableEntry, PteFlags},
};
use config::mm::PAGE_SIZE;
use mm::{
    address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum},
    page_cache::page::Page,
};
use mutex::{ShareMutex, new_share_mutex};
use osfuture::block_on;
use shm::SharedMemory;
use systype::{
    error::{SysError, SysResult},
    memory_flags::MappingFlags,
};
use vfs::file::File;

#[cfg(target_arch = "riscv64")]
use arch::mm::tlb_flush_all_except_global;
#[cfg(target_arch = "riscv64")]
use config::mm::KERNEL_MAP_OFFSET;

use super::page_table::PageTable;

/// A virtual memory area (VMA).
///
/// A VMA is a contiguous, page-aligned region of virtual memory in an address
/// space that has a common set of attributes, such as permissions and mapping type.
#[derive(Clone)]
#[repr(C)]
pub struct VmArea {
    /// Starting virtual address, page-aligned.
    start: VirtAddr,
    /// Ending virtual address (exclusive), page-aligned.
    end: VirtAddr,
    /// Flags of the VMA.
    flags: VmaFlags,
    /// Memory protection of the VMA. Only `RWXU` bits should be set.
    prot: MappingFlags,
    /// Cache for leaf page table entry flags, which are default when a new leaf entry
    /// is created for a shared VMA.
    pte_flags: PteFlags,
    /// Allocated physical pages.
    pages: BTreeMap<VirtPageNum, Arc<Page>>,
    /// Unique data of a specific type of VMA.
    pub map_type: TypedArea,
    /// Page fault handler.
    handler: Option<PageFaultHandler>,
}

/// Unique data of a specific type of VMA. This enum is used in [`VmArea`].
#[derive(Clone, Debug)]
pub enum TypedArea {
    /// A fixed-offset VMA.
    ///
    /// A fixed-offset VMA is used to map physical addresses to virtual addresses
    /// by adding a fixed offset. This is used for kernel space and MMIO regions.
    /// This kind of VMAs do not have a page fault handler, and a page fault will
    /// never occur in this kind of VMAs.
    Offset(OffsetArea),
    /// A file-backed VMA.
    ///
    /// A file-backed VMA is backed by a file. It is created when loading an executable
    /// file or `mmap`ing a file.
    FileBacked(FileBackedArea),
    /// A shared memory VMA.
    ///
    /// A shared memory VMA is created by calling `shmget` and `shmat`. It is backed
    /// by a [`SharedMemory`], which tracks the pages that are shared between processes.
    SharedMemory(SharedMemoryArea),
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
///
/// “Handling a page fault” here means allocating a new page and mapping it to the faulting
/// virtual address by updating the page table. It does not do anything to the TLB.
type PageFaultHandler = fn(&mut VmArea, PageFaultInfo) -> SysResult<()>;

/// Data passed to a page fault handler.
///
/// This struct is used to pass data to a page fault handler registered in a [`VmArea`].
#[derive(Clone, Debug, Copy)]
pub struct PageFaultInfo<'a> {
    /// Faulting virtual address.
    pub fault_addr: VirtAddr,
    /// Page table.
    pub page_table: &'a PageTable,
    /// Type of memory access that caused the page fault. Only one of `R`, `W`, and `X`
    /// can be set.
    pub access: MappingFlags,
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
    /// `prot` needs to have `RWXU` bits set properly; other bits must be zero.
    #[cfg(target_arch = "riscv64")]
    pub fn new_kernel(start_va: VirtAddr, end_va: VirtAddr, prot: MappingFlags) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!((MappingFlags::RWX | MappingFlags::U).contains(prot));

        Self::new_fixed_offset(
            start_va,
            end_va,
            // This field is of no use for kernel VMAs.
            VmaFlags::PRIVATE,
            prot,
            KERNEL_MAP_OFFSET,
        )
    }

    /// Constructs a global [`VmArea`] whose specific type is [`OffsetArea`], representing
    /// an area in the kernel space. This function is used to map a MMIO region.
    ///
    /// `start_va` must be page-aligned.
    ///
    /// `prot` needs to have `RWXU` bits set properly; other bits must be zero.
    #[cfg(target_arch = "riscv64")]
    pub fn new_fixed_offset(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        prot: MappingFlags,
        offset: usize,
    ) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!((MappingFlags::RWX | MappingFlags::U).contains(prot));

        Self {
            start: start_va,
            end: end_va.round_up(),
            flags,
            pte_flags: {
                PteFlags::from(prot | MappingFlags::V | MappingFlags::G) | PteFlags::A | PteFlags::D
            },
            prot,
            pages: BTreeMap::new(),
            map_type: TypedArea::Offset(OffsetArea { offset }),
            handler: None,
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
    /// be equal to `end_va - start_va`. If the file region spans to the end of the file,
    /// `len` must be rounded up to page size.
    ///
    /// To load a segment from an executable file, `offset` and `len` do not need to be
    /// page-aligned, and `len` can be smaller than `end_va - start_va`. If `len` is
    /// smaller than that, the VMA must be private, and the tailing part of the VMA is
    /// filled with zeros. Make sure that the file region defined by `offset` and `len`
    /// is within the file.
    ///
    /// `prot` needs to have `RWX` bits set properly; other bits must be zero.
    pub fn new_file_backed(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        prot: MappingFlags,
        file: Arc<dyn File>,
        offset: usize,
        len: usize,
        seals: Option<MemfdSeals>,
    ) -> Self {
        debug_assert!(
            (len == end_va.to_usize() - start_va.to_usize())
                || ((len < end_va.to_usize() - start_va.to_usize())
                    && flags.contains(VmaFlags::PRIVATE))
        );
        debug_assert!(MappingFlags::RWX.contains(prot));

        let prot = prot | MappingFlags::U;
        Self {
            start: start_va.round_down(),
            end: end_va.round_up(),
            flags,
            pte_flags: {
                let mapping_flags = prot | MappingFlags::V | MappingFlags::U;
                #[cfg(target_arch = "riscv64")]
                {
                    PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    PteFlags::from(mapping_flags)
                }
            },
            prot,
            pages: BTreeMap::new(),
            map_type: TypedArea::FileBacked(FileBackedArea::new(file, offset, len, seals)),
            handler: Some(FileBackedArea::fault_handler),
        }
    }

    /// Constructs a user space [`VmArea`] whose specific type is [`SharedMemoryArea`].
    ///
    /// `start_va` and `end_va` must be page-aligned.
    ///
    /// `prot` should have `RWX` bits set properly; other bits must be zero.
    ///
    /// `shm` is the [`SharedMemory`] to be mapped to the VMA.
    pub fn new_shared_memory(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        prot: MappingFlags,
        shm: ShareMutex<SharedMemory>,
    ) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(MappingFlags::RWX.contains(prot));

        let prot = prot | MappingFlags::U;
        Self {
            start: start_va,
            end: end_va,
            flags,
            pte_flags: {
                let mapping_flags = prot | MappingFlags::V | MappingFlags::U;
                #[cfg(target_arch = "riscv64")]
                {
                    PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    PteFlags::from(mapping_flags)
                }
            },
            prot,
            pages: BTreeMap::new(),
            map_type: TypedArea::SharedMemory(SharedMemoryArea::new(shm)),
            handler: Some(SharedMemoryArea::fault_handler),
        }
    }

    /// Constructs a user space anonymous area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    ///
    /// `prot` needs to have `RWX` bits set properly; other bits must be zero.
    /// Generally, `prot` should have `RW` bits set, because an anonymous area which
    /// cannot be written is useless.
    pub fn new_anonymous(
        start_va: VirtAddr,
        end_va: VirtAddr,
        flags: VmaFlags,
        prot: MappingFlags,
    ) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(MappingFlags::RWX.contains(prot));

        let prot = prot | MappingFlags::U;
        Self {
            start: start_va,
            end: end_va,
            flags,
            pte_flags: {
                let mapping_flags = prot | MappingFlags::V | MappingFlags::U;
                #[cfg(target_arch = "riscv64")]
                {
                    PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    PteFlags::from(mapping_flags)
                }
            },
            prot,
            pages: BTreeMap::new(),
            map_type: TypedArea::Anonymous(AnonymousArea::new(flags)),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Constructs a user space stack area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_stack(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);

        let flags = VmaFlags::PRIVATE;
        Self {
            start: start_va,
            end: end_va,
            flags,
            pte_flags: {
                let mapping_flags =
                    MappingFlags::V | MappingFlags::R | MappingFlags::W | MappingFlags::U;
                #[cfg(target_arch = "riscv64")]
                {
                    PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    PteFlags::from(mapping_flags)
                }
            },
            prot: MappingFlags::R | MappingFlags::W | MappingFlags::U,
            pages: BTreeMap::new(),
            map_type: TypedArea::Anonymous(AnonymousArea::new(flags)),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Constructs a user space heap area.
    ///
    /// `start_va` and `end_va` must be page-aligned.
    pub fn new_heap(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        debug_assert!(start_va.to_usize() % PAGE_SIZE == 0);
        debug_assert!(end_va.to_usize() % PAGE_SIZE == 0);

        let flags = VmaFlags::PRIVATE;
        Self {
            start: start_va,
            end: end_va,
            flags,
            pte_flags: {
                let mapping_flags =
                    MappingFlags::V | MappingFlags::R | MappingFlags::W | MappingFlags::U;
                #[cfg(target_arch = "riscv64")]
                {
                    PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    PteFlags::from(mapping_flags)
                }
            },
            prot: MappingFlags::R | MappingFlags::W | MappingFlags::U,
            pages: BTreeMap::new(),
            map_type: TypedArea::Heap(AnonymousArea::new(flags)),
            handler: Some(AnonymousArea::fault_handler),
        }
    }

    /// Splits a virtual memory area at the given boundaries.
    ///
    /// The area is split at `split_start` and `split_end`, creating three potential areas:
    /// 1. Area before `split_start` (exclusive)
    /// 2. Area between `split_start` (inclusive) and `split_end` (exclusive)
    /// 3. Area after `split_end` (inclusive)
    ///
    /// `split_start` and `split_end` must both be page-aligned, with `split_start < split_end`.
    /// If the boundaries are outside the VMA range, they're clamped to the VMA's boundaries.
    ///
    /// Note that the returned [`VmArea`]s may have page table entries mapped in its associated
    /// page table, but these entries are not invalidated when any of the [`VmArea`]s are dropped.
    /// Make sure to call [`Self::unmap_area`] on a [`VmArea`] which is to be dropped to invalidate
    /// the page table entries.
    ///
    /// Returns a tuple of three items:
    /// - The area before the split range (None if `split_start` is at or before the VMA start)
    /// - The area in the split range (None if the split range doesn't overlap the VMA)
    /// - The area after the split range (None if `split_end` is at or after the VMA end)
    pub fn split_area(
        mut self,
        split_start: VirtAddr,
        split_end: VirtAddr,
    ) -> (Option<Self>, Option<Self>, Option<Self>) {
        debug_assert!(split_start < split_end);
        debug_assert!(split_start.to_usize() % PAGE_SIZE == 0);
        debug_assert!(split_end.to_usize() % PAGE_SIZE == 0);

        let split_start = VirtAddr::max(self.start, split_start);
        let split_end = VirtAddr::min(self.end, split_end);

        if split_start >= split_end {
            // The range to be split does not overlap with the VMA.
            return if split_start >= self.end {
                (Some(self), None, None)
            } else {
                (None, None, Some(self))
            };
        }

        // Re-assign `Page`s in the old VMA to the new VMA(s).
        let (pages_low, pages_mid, pages_high) = {
            let mut pages = mem::take(&mut self.pages);
            let mut pages_mid_high = pages.split_off(&split_start.page_number());
            let pages_high = pages_mid_high.split_off(&split_end.page_number());
            (pages, pages_mid_high, pages_high)
        };

        let vma_low = if split_start > self.start {
            let mut vma = self.clone();
            vma.end = split_start;
            vma.pages = pages_low;
            if let TypedArea::FileBacked(file_backed) = &mut vma.map_type {
                file_backed.len = cmp::min(
                    file_backed.len,
                    split_start.to_usize() - vma.start.to_usize(),
                );
            }
            Some(vma)
        } else {
            None
        };

        let vma_mid = if split_start < split_end {
            let mut vma = self.clone();
            vma.start = split_start;
            vma.end = split_end;
            vma.pages = pages_mid;
            if let TypedArea::FileBacked(file_backed) = &mut vma.map_type {
                let start_offset = split_start.to_usize() - self.start.to_usize();
                file_backed.offset += start_offset;
                file_backed.len = cmp::min(
                    file_backed.len.saturating_sub(start_offset),
                    split_end.to_usize() - split_start.to_usize(),
                );
            }
            Some(vma)
        } else {
            None
        };

        let vma_high = if split_end < self.end {
            let mut vma = self.clone();
            vma.start = split_end;
            vma.pages = pages_high;
            if let TypedArea::FileBacked(file_backed) = &mut vma.map_type {
                let start_offset = split_end.to_usize() - self.start.to_usize();
                file_backed.len = file_backed.len.saturating_sub(start_offset);
                file_backed.offset += start_offset;
            }
            Some(vma)
        } else {
            None
        };

        (vma_low, vma_mid, vma_high)
    }

    /// Unmaps a virtual memory area from the given page table.
    ///
    /// This function invalidates all valid page table entries in the VMA, and drops
    /// the `VmArea` itself. This is the proper way to drop a `VmArea` which is
    /// associated with a [`AddrSpace`].
    pub fn unmap_area(mut self, page_table: &PageTable) {
        for (vpn, _) in mem::take(&mut self.pages) {
            let pte = page_table.find_entry(vpn).unwrap();
            *pte = PageTableEntry::default();
        }
        tlb_shootdown(self.start_va().to_usize(), self.length());
    }

    /// Changes the protection flags of a user space VMA, possibly updating page table
    /// entries.
    ///
    /// `new_prot` needs to have `RWX` bits set properly; other bits must be zero. This
    /// function cannot change the `U` bit.
    pub fn change_prot(&mut self, page_table: &PageTable, new_prot: MappingFlags) {
        debug_assert!(MappingFlags::RWX.contains(new_prot));

        let old_prot = self.prot;
        self.prot = new_prot | MappingFlags::U;
        self.pte_flags = {
            let mapping_flags = self.prot | MappingFlags::V;
            #[cfg(target_arch = "riscv64")]
            {
                PteFlags::from(mapping_flags) | PteFlags::A | PteFlags::D
            }
            #[cfg(target_arch = "loongarch64")]
            {
                PteFlags::from(mapping_flags)
            }
        };
        for &vpn in self.pages.keys() {
            let pte = page_table.find_entry(vpn).unwrap();
            pte.set_flags(self.pte_flags);
        }
        // Flush the TLB if any kind of permission is downgraded.
        if !new_prot.contains(old_prot) {
            tlb_shootdown(self.start_va().to_usize(), self.length());
        }
    }

    /// Handles a page fault happened in this VMA.
    ///
    /// # Errors
    /// Returns [`SysError::EFAULT`] if the access permission is not allowed.
    /// Otherwise, returns [`SysError::ENOMEM`] if a new frame cannot be allocated.
    pub fn handle_page_fault(&mut self, info: PageFaultInfo) -> SysResult<()> {
        let &mut VmArea {
            pte_flags, prot, ..
        } = self;
        let PageFaultInfo {
            fault_addr,
            page_table,
            access,
        } = info;

        // Check the protection bits.
        if !prot.contains(access) {
            log::error!(
                "VmArea::handle_page_fault: access {:?} at {:#x} not allowed, with protection {:?}",
                access,
                fault_addr.to_usize(),
                prot,
            );
            return Err(SysError::EFAULT);
        }

        let pte = {
            #[allow(unused_variables)]
            let (pte, flush_all) =
                page_table.find_entry_force(fault_addr.page_number(), pte_flags)?;
            #[cfg(target_arch = "riscv64")]
            if flush_all {
                tlb_flush_all_except_global();
            }
            pte
        };

        // log::error!(
        //     "current task address: {:?}, pid: {}",
        //     Arc::as_ptr(&current_task()) as *const usize,
        //     current_task().tid()
        // );
        // log::error!("handle_page_fault {:?}", info.fault_addr);
        if pte.is_valid() {
            if access == MappingFlags::W
                && !MappingFlags::from(pte.flags()).contains(MappingFlags::W)
            {
                // Copy-on-write page fault.
                self.handle_cow_fault(fault_addr, pte)?;
            } else {
                // The page is already mapped by another thread, so just flush the TLB.
            }
        } else {
            // log::warn!("handle_fault: pte not valid");
            self.handler.unwrap()(self, info)?;
        }
        tlb_flush_addr(fault_addr.to_usize());

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
        // log::error!("[handle_cow_fault] fault_addr: {:?}", fault_addr);
        let fault_vpn = fault_addr.page_number();
        let fault_page = self.pages.get(&fault_vpn).unwrap();
        if Arc::strong_count(fault_page) > 1 {
            // Allocate a new page and copy the content if the page is shared.
            let new_page = Page::build()?;
            new_page.copy_from_page(fault_page);

            let mut new_pte = *pte;

            #[cfg(target_arch = "riscv64")]
            let new_flags = new_pte.flags().union(PteFlags::W);
            #[cfg(target_arch = "loongarch64")]
            let new_flags = new_pte.flags().union(PteFlags::W | PteFlags::D);

            new_pte.set_flags(new_flags);
            new_pte.set_ppn(new_page.ppn());
            *pte = new_pte;

            self.pages.insert(fault_vpn, Arc::new(new_page));
        } else {
            // Just set the write bit if the page is not shared.
            let mut new_pte = *pte;

            #[cfg(target_arch = "riscv64")]
            let new_flags = new_pte.flags().union(PteFlags::W);
            #[cfg(target_arch = "loongarch64")]
            let new_flags = new_pte.flags().union(PteFlags::W | PteFlags::D);

            new_pte.set_flags(new_flags);
            *pte = new_pte;
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

    /// Checks seal permission in fileback Area specially.
    ///
    /// Returns Ok when seal eqs to `prot`.Otherwise, return EPERM.
    /// Now `Write` is supported only.
    pub fn check_seals(&self, prot: MappingFlags) -> SysResult<()> {
        if !prot.contains(MappingFlags::W) {
            return Ok(());
        }

        use crate::vm::vm_area::TypedArea::FileBacked;
        if let FileBacked(area) = &self.map_type {
            if let Some(seals) = area.seals {
                // log::error!("area: {:?}, seals: {:?}", area, seals);
                if seals.contains(MemfdSeals::WRITE) {
                    return Err(SysError::EPERM);
                }
            }
        }
        Ok(())
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

    /// Returns the length of the VMA in bytes.
    pub fn length(&self) -> usize {
        self.end.to_usize() - self.start.to_usize()
    }

    /// Returns the flags of the VMA.
    pub fn flags(&self) -> VmaFlags {
        self.flags
    }

    /// Returns the PTE flags of the VMA.
    pub fn pte_flags(&self) -> PteFlags {
        self.pte_flags
    }

    /// Returns the protection flags of the VMA.
    pub fn prot(&self) -> MappingFlags {
        self.prot
    }

    /// Returns the mapping from virtual page numbers to `Arc<Page>`s mapped in this VMA.
    pub fn pages(&self) -> &BTreeMap<VirtPageNum, Arc<Page>> {
        &self.pages
    }

    /// Returns whether this VMA is a heap.
    pub fn is_heap(&self) -> bool {
        matches!(self.map_type, TypedArea::Heap(_))
    }

    /// Returns whether this VMA is a shared memory area.
    pub fn is_shared_memory(&self) -> bool {
        matches!(self.map_type, TypedArea::SharedMemory(_))
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
#[derive(Clone, Debug)]
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
///   in the page cache of the file, depending on whether the page have been written to
///   or not. Assuming the page is first read from and then written to, the behavior is
///   as follows:
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
    /// The memseals of the mapped region in the file (if file is memfd).
    seals: Option<MemfdSeals>,
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
    pub fn new(file: Arc<dyn File>, offset: usize, len: usize, seals: Option<MemfdSeals>) -> Self {
        let offset_aligned = offset / PAGE_SIZE * PAGE_SIZE;
        let len_aligned = len + offset % PAGE_SIZE;
        Self {
            file,
            offset: offset_aligned,
            len: len_aligned,
            seals,
        }
    }

    /// Handles a page fault.
    fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let &FileBackedArea {
            ref file,
            offset,
            len: region_len,
            ..
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
        // log::error!("[FileBackedArea] fault_addr: {:?}", fault_addr);

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
        let cached_page = block_on(async { file.read_page(file_offset).await })?;

        if area_offset + PAGE_SIZE > region_len {
            // Part of the page is in the file region, and part of the page is not.
            // We allocate a new page and copy the first part of the page from the
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
        let page = if flags.contains(VmaFlags::PRIVATE) && access == MappingFlags::W {
            // Write to a private VMA: Do copy-on-write beforehand.
            let page = Page::build()?;
            page.copy_from_page(&cached_page);
            Arc::new(page)
        } else {
            // Other conditions: Just use the cached page.
            if flags.contains(VmaFlags::PRIVATE) {
                // Read from or execute a private VMA: Make sure we mark the page as read-only.
                #[cfg(target_arch = "riscv64")]
                {
                    pte_flags = pte_flags.difference(PteFlags::W);
                }
                #[cfg(target_arch = "loongarch64")]
                {
                    pte_flags = pte_flags.difference(PteFlags::W | PteFlags::D);
                }
            }
            cached_page
        };
        page_table.map_page_to(fault_addr.page_number(), page.ppn(), pte_flags)?;
        area.pages.insert(fault_addr.page_number(), page);
        Ok(())
    }

    /// Returns the file backing store.
    pub fn file(&self) -> &Arc<dyn File> {
        &self.file
    }

    /// Returns the offset of the mapped region in the file.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the length of the mapped region in the file.
    pub fn len(&self) -> usize {
        self.len
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

/// A shared memory area.
///
/// This struct is used to map a shared memory object to a user process.
#[derive(Clone, Debug)]
pub struct SharedMemoryArea {
    /// The shared memory object.
    shm: ShareMutex<SharedMemory>,
}

impl SharedMemoryArea {
    /// Creates a new shared memory area.
    pub fn new(shm: ShareMutex<SharedMemory>) -> Self {
        Self { shm }
    }

    /// Handles a page fault.
    fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let SharedMemoryArea { shm } = match &area.map_type {
            TypedArea::SharedMemory(pages_backed) => pages_backed,
            _ => panic!("PagesBackedArea::fault_handler: not a pages-backed area"),
        };
        let &mut VmArea {
            start: start_va,
            pte_flags,
            ..
        } = area;
        let PageFaultInfo {
            fault_addr,
            page_table,
            ..
        } = info;

        log::warn!(
            "SharedMemoryArea::fault_handler: page fault at {:#x} in shared memory area",
            fault_addr.to_usize()
        );

        let area_offset = fault_addr.round_down().to_usize() - start_va.to_usize();
        let page_index = area_offset / PAGE_SIZE;
        let page_num = fault_addr.page_number();

        let pages = &mut shm.lock().pages;
        let page = match &pages[page_index] {
            Some(page) => {
                log::warn!("Found page at index {:#x}", page_index);
                Arc::clone(page)
            }
            None => {
                // Allocate a new page and fill it with zeros.
                log::warn!("Allocating new page at index {:#x}", page_index);
                let page = Arc::new(Page::build()?);
                page.as_mut_slice().fill(0);
                pages[page_index] = Some(Arc::clone(&page));
                page
            }
        };
        page_table.map_page_to(page_num, page.ppn(), pte_flags)?;
        area.pages.insert(page_num, page);

        Ok(())
    }
}

/// An anonymous VMA which is not backed by a file or device, such as a user heap or stack.
///
/// Each anonymous VMA is filled with zeros when a process first accesses it, in order to
/// avoid leaking data from other processes or the kernel. This is similar to the
/// `.bss` section in an executable file.
#[derive(Clone, Debug)]
pub struct AnonymousArea {
    /// The mappings from virtual page numbers to physical pages, if the area is shared.
    mappings: Option<ShareMutex<BTreeMap<VirtPageNum, Arc<Page>>>>,
}

impl AnonymousArea {
    /// Creates a new anonymous area with the given flags.
    fn new(flags: VmaFlags) -> Self {
        let mappings = if flags.contains(VmaFlags::PRIVATE) {
            None
        } else {
            Some(new_share_mutex(BTreeMap::new()))
        };
        Self { mappings }
    }

    /// Handles a page fault.
    fn fault_handler(area: &mut VmArea, info: PageFaultInfo) -> SysResult<()> {
        let mappings = match &area.map_type {
            TypedArea::Anonymous(anonymous) => anonymous.mappings.as_ref(),
            TypedArea::Heap(anonymous) => None,
            _ => panic!("AnonymousArea::fault_handler: not an anonymous area or a heap area"),
        };
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

        let vpn = fault_addr.page_number();
        let page = if flags.contains(VmaFlags::PRIVATE) {
            let page = Page::build()?;
            page.as_mut_slice().fill(0);
            Arc::new(page)
        } else {
            let mut mappings_lock = mappings.unwrap().lock();
            match mappings_lock.get(&vpn).cloned() {
                Some(page) => page,
                None => {
                    let page = Arc::new(Page::build()?);
                    page.as_mut_slice().fill(0);
                    mappings_lock.insert(vpn, Arc::clone(&page));
                    page
                }
            }
        };
        page_table.map_page_to(vpn, page.ppn(), pte_flags)?;
        pages.insert(vpn, page);

        Ok(())
    }
}
