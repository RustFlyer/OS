//! Module for manipulating page tables and manage memory mappings.
//!
//! This module provides a `PageTable` struct that manipulates page tables in momery
//! and tracking allocated pages.

use alloc::vec::Vec;

#[cfg(target_arch = "loongarch64")]
use loongArch64::register::pgdl;

use arch::mm::{fence, switch_pagetable, tlb_shootdown};
use config::mm::{KERNEL_MAP_OFFSET, PAGE_SIZE, PTE_PER_TABLE};
use mm::{
    address::{PhysPageNum, VirtAddr, VirtPageNum},
    page_cache::page::Page,
};
use mutex::SpinLock;
use systype::{SysError, SysResult};

#[cfg(target_arch = "riscv64")]
use arch::mm::tlb_flush_all_except_global;
#[cfg(target_arch = "riscv64")]
use config::mm::{
    MMIO_END, MMIO_PHYS_RANGES, MMIO_START, VIRT_END, bss_end, bss_start, data_end, data_start,
    kernel_end, kernel_start, rodata_end, rodata_start, text_end, text_start, trampoline_end,
    trampoline_start,
};

use super::pte::{PageTableEntry, PteFlags};
use crate::frame::FrameTracker;

#[cfg(target_arch = "riscv64")]
use super::mapping_flags::MappingFlags;
#[cfg(target_arch = "riscv64")]
use crate::vm::vm_area::{OffsetArea, VmArea};

/// A data structure for manipulating page tables and manage memory mappings.
///
/// This struct represents a page table (with its sub page tables) in the memory.
/// It provides methods to manipulate page tables and page table entries in a
/// convenient way. It also tracks allocated frames for the page table itself
/// and allocatable frames mapped by the page table.
///
/// A page table consists of page tables and mapped physical frames. Both of them
/// can be separated into two categories: kernel-used and user-used. Kernel-used
/// tables and frames are shared among all processes, and is not tracked by
/// this struct. User-used tables are exclusively allocated for each process,
/// and are tracked by this struct. When a `PageTable` is dropped, all user-used
/// tables are dropped.
#[derive(Debug)]
pub struct PageTable {
    /// Physical page number of the root page table.
    root: PhysPageNum,
    /// Frames allocated for user-used tables
    frames: SpinLock<Vec<FrameTracker>>,
}

#[cfg(target_arch = "riscv64")]
lazy_static! {
    /// The kernel page table.
    pub static ref KERNEL_PAGE_TABLE: PageTable = unsafe { PageTable::build_kernel_page_table() };
}

impl PageTable {
    /// Builds a new `PageTable` with an empty root page table.
    ///
    /// # Errors
    /// Returns an [`ENOMEM`] error if memory allocation for the root page table fails.
    pub fn build() -> SysResult<Self> {
        let root_frame = FrameTracker::build()?;
        // SAFETY: the frame is newly allocated for the root page table.
        unsafe {
            PageTableMem::new(root_frame.ppn()).clear();
        }
        Ok(PageTable {
            root: root_frame.ppn(),
            frames: SpinLock::new(alloc::vec![root_frame]),
        })
    }

    /// Constructs the kernel page table.
    ///
    /// The kernel page table is a page table that maps the entire kernel space.
    /// The mapping is linear, i.e., VPN = PPN + KERNEL_MAP_OFFSET.
    ///
    /// # Safety
    /// This function should be called only once during the kernel initialization.
    ///
    /// # Panics
    /// Panics if the kernel page table cannot be constructed due to lack of free
    /// frames, which should not happen in practice.
    #[cfg(target_arch = "riscv64")]
    unsafe fn build_kernel_page_table() -> Self {
        let mut page_table = Self::build().expect("out of memory");

        /* Map the kernel's .text, .rodata, .data, and .bss sections */

        let text_start_va = VirtAddr::new(text_start());
        let text_end_va = VirtAddr::new(text_end());
        let text_prot = MappingFlags::R | MappingFlags::X;
        let text_vma = VmArea::new_kernel(text_start_va, text_end_va, text_prot);
        OffsetArea::map(&text_vma, &mut page_table);

        // Map the kernel's signal handling trampoline.
        let trampoline_start_va = VirtAddr::new(trampoline_start());
        let trampoline_end_va = VirtAddr::new(trampoline_end());
        let trampoline_prot = MappingFlags::R | MappingFlags::X | MappingFlags::U;
        let trampoline_vma =
            VmArea::new_kernel(trampoline_start_va, trampoline_end_va, trampoline_prot);
        OffsetArea::map(&trampoline_vma, &mut page_table);

        let rodata_start_va = VirtAddr::new(rodata_start());
        let rodata_end_va = VirtAddr::new(rodata_end());
        let rodata_prot = MappingFlags::R;
        let rodata_vma = VmArea::new_kernel(rodata_start_va, rodata_end_va, rodata_prot);
        OffsetArea::map(&rodata_vma, &mut page_table);

        let data_start_va = VirtAddr::new(data_start());
        let data_end_va = VirtAddr::new(data_end());
        let data_prot = MappingFlags::R | MappingFlags::W;
        let data_vma = VmArea::new_kernel(data_start_va, data_end_va, data_prot);
        OffsetArea::map(&data_vma, &mut page_table);

        let bss_start_va = VirtAddr::new(bss_start());
        let bss_end_va = VirtAddr::new(bss_end());
        let bss_prot = MappingFlags::R | MappingFlags::W;
        let bss_vma = VmArea::new_kernel(bss_start_va, bss_end_va, bss_prot);
        OffsetArea::map(&bss_vma, &mut page_table);

        /* Map the allocatable frames */
        let alloc_start_va = VirtAddr::new(kernel_end());
        let alloc_end_va = VirtAddr::new(VIRT_END);
        let alloc_prot = MappingFlags::R | MappingFlags::W;
        let alloc_vma = VmArea::new_kernel(alloc_start_va, alloc_end_va, alloc_prot);
        OffsetArea::map(&alloc_vma, &mut page_table);

        /* Map memory-mapped I/O */
        let mmio_prot = MappingFlags::R | MappingFlags::W;
        for &(start_pa, len) in MMIO_PHYS_RANGES {
            let mmio_start_va = VirtAddr::new(start_pa + KERNEL_MAP_OFFSET);
            let mmio_end_va = VirtAddr::new(start_pa + len + KERNEL_MAP_OFFSET);
            let mmio_vma = VmArea::new_kernel(mmio_start_va, mmio_end_va, mmio_prot);
            OffsetArea::map(&mmio_vma, &mut page_table);
        }

        unsafe {
            log::error!(
                "todo! when kernel pagetable init, DTB_ADDR is {:#x}",
                unsafe { DTB_ADDR }
            );
            /* Map dtb memory*/
            let offset = DTB_START - DTB_ADDR;
            let dtb_start = VirtAddr::new(DTB_START);
            let dtb_end = VirtAddr::new(DTB_END);

            let dtb_prot = MappingFlags::RWX;
            let dtb_vma =
                VmArea::new_fixed_offset(dtb_start, dtb_end, VmaFlags::PRIVATE, dtb_prot, offset);

            OffsetArea::map(&dtb_vma, &mut page_table);
        }

        page_table
    }

    /// Gets the physical page number of the root page table.
    pub fn root(&self) -> PhysPageNum {
        self.root
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// This method creates absent non-leaf entries using `inner_flags`. Note that
    /// the returned entry may be invalid.
    ///
    /// `inner_flags` decides the flags for non-leaf entries. Some bits may not be
    /// used.
    ///
    /// This function only support 4 KiB pages, 3-level page tables.
    ///
    /// Returns a mutable reference to the leaf page table entry, and a boolean
    /// indicating whether any non-leaf entry is created. If any non-leaf entry is
    /// created, `sfence.vma` on a specific address is not enough to ensure the
    /// non-leaf entry is visible to the hart; use `sfence_vma(0, 0)` if that is
    /// the case.
    ///
    /// Returns an [`ENOMEM`] error if the method needs to allocate a frame but fails
    /// to do so.
    pub fn find_entry_force(
        &self,
        vpn: VirtPageNum,
        inner_flags: PteFlags,
    ) -> SysResult<(&mut PageTableEntry, bool)> {
        let mut ppn = self.root;
        let inner_flags = {
            #[cfg(target_arch = "riscv64")]
            {
                (inner_flags & PteFlags::G) | PteFlags::V
            }
            #[cfg(target_arch = "loongarch64")]
            {
                // TODO: Check the required bits for LoongArch
                (inner_flags & PteFlags::G) | PteFlags::V
            }
        };
        let mut inner_created = false;
        for (i, index) in vpn.indices().into_iter().enumerate().rev() {
            let mut page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry_mut(index);
            if i == 0 {
                return Ok((entry, inner_created));
            }
            if !entry.is_valid() {
                let frame = FrameTracker::build()?;
                unsafe {
                    PageTableMem::new(frame.ppn()).clear();
                }
                *entry = PageTableEntry::new(frame.ppn(), inner_flags);
                self.track_frame(frame);
                inner_created = true;
            }
            ppn = entry.ppn();
        }
        unreachable!();
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// If any non-leaf entry is not present, returns `None`. Note that the returned
    /// entry may be invalid.
    ///
    /// This function only support 4 KiB pages, 3-level page tables.
    pub fn find_entry(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let mut ppn = self.root;
        for (i, index) in vpn.indices().into_iter().enumerate().rev() {
            let mut page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry_mut(index);
            if i == 0 {
                return Some(entry);
            }
            if !entry.is_valid() {
                return None;
            }
            ppn = entry.ppn();
        }
        unreachable!();
    }

    /// Maps a leaf page by specifying VPN and page table entry flags, to a newly
    /// allocated frame.
    ///
    /// This method allocates a frame for the leaf page and sets the mapping in the
    /// page table
    ///
    /// Returns a [`SysResult`] indicating whether the operation is successful.
    /// Returns an [`ENOMEM`] error if the method needs to allocate a frame but fails
    /// to do so.
    ///
    /// In the successful case, if the VPN is already mapped, the function returns
    /// a mutable reference to the existing page table entry wrapped as
    /// `SysResult::Ok(Err(entry))`. Otherwise, the function allocates a new frame
    /// for it and returns the new page wrapped as `SysResult::Ok(Ok(page))`.
    ///
    /// # Note for RISC-V
    /// This function takes `flags` as the flags for the leaf page table entry, and
    /// it takes bits `G` to set the flags for intermediate entries. This design
    /// provides a lower granularity of control over intermediate entries, but it is
    /// sufficient for the current address space layout.
    pub fn map_page(
        &self,
        vpn: VirtPageNum,
        flags: PteFlags,
    ) -> SysResult<Result<Page, &mut PageTableEntry>> {
        #[allow(unused)]
        let (entry, non_leaf_created) = self.find_entry_force(vpn, flags)?;
        if entry.is_valid() {
            return Ok(Err(entry));
        }
        let page = Page::build()?;
        *entry = PageTableEntry::new(page.ppn(), flags);

        #[cfg(target_arch = "riscv64")]
        if non_leaf_created {
            tlb_flush_all_except_global();
        }

        Ok(Ok(page))
    }

    /// Maps a leaf page by specifying VPN, PPN, and page table entry flags.
    ///
    /// This method does not allocate the frame for the leaf page. It only sets the
    /// mapping in the page table. The caller should allocate a frame is allocated
    /// and set the mapping by calling this method.
    ///
    /// Returns a [`SysResult`] indicating whether the operation is successful.
    /// Returns an [`ENOMEM`] error if the method needs to allocate a frame but fails
    /// to do so.
    ///
    /// # Errors
    /// Returns an [`EINVAL`] error if the page is already mapped. Returns an [`ENOMEM`]
    /// errors if the method needs to allocate a frame but fails to do so.
    ///
    /// # Note
    /// This function takes `flags` as the flags for the leaf page table entry, and
    /// it takes bits `G` to set the flags for intermediate entries. This design
    /// provides a lower granularity of control over intermediate entries, but it is
    /// sufficient for the current address space layout.
    pub fn map_page_to(
        &self,
        vpn: VirtPageNum,
        ppn: PhysPageNum,
        flags: PteFlags,
    ) -> SysResult<()> {
        #[allow(unused)]
        let (entry, non_leaf_created) = self.find_entry_force(vpn, flags)?;
        if entry.is_valid() {
            return Err(SysError::EINVAL);
        }
        *entry = PageTableEntry::new(ppn, flags);

        #[cfg(target_arch = "riscv64")]
        if non_leaf_created {
            tlb_flush_all_except_global();
        }

        Ok(())
    }

    /// Unmaps a leaf page by specifying the VPN.
    ///
    /// This method does not deallocate the frame for the leaf page. It only clears the
    /// mapping in the page table. The caller should deallocate the frame and clear
    /// the mapping by calling this method. Calling this method to clear an unmapped
    /// page is safe.
    pub fn unmap_page(&self, vpn: VirtPageNum) {
        if let Some(entry) = self.find_entry(vpn) {
            *entry = PageTableEntry::default();
            // Flush TLB entries for the page for all harts.
            // We can optimize this by only flushing the TLB for harts that execute
            // the current process.
            fence();
            tlb_shootdown(vpn.address().to_usize(), 1);
        }
    }

    /// Maps a range of leaf pages by specifying the starting VPN, corresponsing PPNs,
    /// and page table entry flags.
    ///
    /// The range is `[start_vpn, start_vpn + ppns.len())`. The range must be valid
    /// and not overlap with existing mappings. The range length must not be zero.
    ///
    /// This methods does not allocate the frames for the leaf pages. It only sets
    /// the mappings in the page table. The caller should allocate frames and set the
    /// mappings by calling this method. Be careful that calling this method with an
    /// already mapped `vpn` will overwrite the existing mapping.
    ///
    /// # Note
    /// By the current implementation, any non-leaf entry is created with the same
    /// `G` bit as the leaf entries. This design is sufficient for the current
    /// address space layout.
    pub fn map_range_to(
        &self,
        start_vpn: VirtPageNum,
        ppns: &[PhysPageNum],
        flags: PteFlags,
    ) -> SysResult<()> {
        // Optimization is applied to cut down redundant lookups to entries in the
        // same leaf page table.
        let mut entry = self.find_entry_force(start_vpn, flags)?.0;
        *entry = PageTableEntry::new(ppns[0], flags);
        for (i, &ppn) in ppns.iter().enumerate().skip(1) {
            let vpn = start_vpn.to_usize() + i;
            entry = if vpn % PTE_PER_TABLE == 0 {
                self.find_entry_force(VirtPageNum::new(vpn), flags)?.0
            } else {
                // SAFETY: the entry is not the last one in its page table, so the
                // next entry of `entry` is valid.
                unsafe { &mut *(entry as *mut PageTableEntry).add(1) }
            };
            *entry = PageTableEntry::new(ppn, flags);
        }

        // Simply flush all TLB entries, as the range is likely to be large.
        #[cfg(target_arch = "riscv64")]
        tlb_flush_all_except_global();

        Ok(())
    }

    /// Unmaps a range of leaf pages by specifying the starting VPN and the number of pages.
    ///
    /// The range is `[start_vpn, start_vpn + count)`.
    ///
    /// This method does not deallocate the frames for the leaf pages. It only clears the
    /// mappings in the page table. The caller should deallocate the frames and clear the
    /// mappings by calling this method. Calling this method to clear any unmapped range
    /// is safe.
    pub fn unmap_range(&self, start_vpn: VirtPageNum, count: usize) {
        for i in 0..count {
            let vpn = VirtPageNum::new(start_vpn.to_usize() + i);
            self.unmap_page(vpn);
        }
        // Perfrom a TLB shootdown for the range.
        // TODO: Optimize this by only flushing the TLB for harts that execute the current
        // process.
        fence();
        tlb_shootdown(start_vpn.address().to_usize(), count);
    }

    /// Maps the kernel part of the address space into this page table.
    ///
    /// This method is used to map the kernel space into a new page table for a user process.
    /// This method does not allocate any frame or make this page table own any frame.
    #[cfg(target_arch = "riscv64")]
    pub fn map_kernel(&self) {
        // Map the kernel areas.
        let kernel_vpn_start = VirtAddr::new(kernel_start()).page_number();
        let kernel_vpn_end = VirtAddr::new(kernel_end()).page_number();
        // Range of the top-level PTEs that covers the kernel space.
        let index_start = kernel_vpn_start.indices()[2];
        let index_end = kernel_vpn_end.indices()[2];

        let mut page_table = unsafe { PageTableMem::new(self.root) };
        let kernel_page_table = unsafe { PageTableMem::new(KERNEL_PAGE_TABLE.root) };
        let src = &kernel_page_table.as_slice()[index_start..=index_end];
        let dst = &mut page_table.as_slice_mut()[index_start..=index_end];
        dst.copy_from_slice(src);

        // Map the memory-mapped I/O space.
        let mmio_vpn_start = VirtAddr::new(MMIO_START).page_number();
        let mmio_vpn_end = VirtAddr::new(MMIO_END).page_number();
        let index_start = mmio_vpn_start.indices()[2];
        let index_end = mmio_vpn_end.indices()[2];

        let src = &kernel_page_table.as_slice()[index_start..=index_end];
        let dst = &mut page_table.as_slice_mut()[index_start..=index_end];
        dst.copy_from_slice(src);
    }

    /// Adds a `FrameTracker` to the page table so that the frame can be deallocated
    /// when the `PageTable` is dropped. Any page table frame in the page itself
    /// table must be tracked by calling this method.
    fn track_frame(&self, frame: FrameTracker) {
        self.frames.lock().push(frame);
    }

    /// Map the physical addresses of I/O memory resources to core virtual
    /// addresses.
    ///
    /// Linux also has this function.
    pub fn ioremap(&self, paddr: usize, size: usize) -> SysResult<()> {
        let flags = PteFlags::V | PteFlags::W | PteFlags::D;
        let mut vpn = VirtAddr::new(paddr + KERNEL_MAP_OFFSET)
            .round_down()
            .page_number();
        let mut ppn = vpn.to_ppn_kernel();
        let mut size = size as isize;
        while size > 0 {
            self.map_page_to(vpn, ppn, flags)?;
            vpn = VirtPageNum::new(vpn.to_usize() + 1);
            ppn = PhysPageNum::new(ppn.to_usize() + 1);
            size -= PAGE_SIZE as isize;
        }
        Ok(())
    }

    pub fn iounmap(&self, vaddr: usize, size: usize) {
        let mut vpn = VirtAddr::new(vaddr).page_number();
        let mut size = size as isize;
        while size > 0 {
            self.unmap_page(vpn);
            vpn = VirtPageNum::new(vpn.to_usize() + 1);
            size -= PAGE_SIZE as isize;
        }
    }
}

/// A helper struct for manipulating a page table in memory temporarily.
///
/// # Discussion
/// To achieve thread-safe access to a page table, we need to ensure that only one
/// thread can get a mutable reference to the page table at a time. Consider using
/// locks to protect the page table. We should change the implementation of this struct
/// in the future.
#[derive(Debug)]
struct PageTableMem {
    /// Physical page number of the page table.
    ppn: PhysPageNum,
}

impl PageTableMem {
    /// Constructs a new `PageTableMem` from the given physical address.
    ///
    /// # Safety
    /// The given address must point to a valid page table.
    /// The constructed value must not be used after the page table is deallocated.
    unsafe fn new(ppn: PhysPageNum) -> Self {
        PageTableMem { ppn }
    }

    fn as_slice(&self) -> &'static [PageTableEntry; PTE_PER_TABLE] {
        // SAFETY: the page `ppn` points to is a valid page table thus allocated.
        unsafe { &*(self.ppn.to_vpn_kernel().as_slice().as_ptr() as *const _) }
    }

    fn as_slice_mut(&mut self) -> &'static mut [PageTableEntry; PTE_PER_TABLE] {
        // SAFETY: the page `ppn` points to is a valid page table thus allocated.
        unsafe { &mut *(self.ppn.to_vpn_kernel().as_slice_mut().as_mut_ptr() as *mut _) }
    }

    /// Gets the entry at the given index.
    fn get_entry(&self, index: usize) -> &'static PageTableEntry {
        &self.as_slice()[index]
    }

    /// Gets the entry at the given index mutably.
    fn get_entry_mut(&mut self, index: usize) -> &'static mut PageTableEntry {
        &mut self.as_slice_mut()[index]
    }

    /// Clears the page table.
    fn clear(&mut self) {
        self.as_slice_mut().fill(PageTableEntry::default());
    }
}

/// Switch to the kernel page table.
///
/// # Safety
/// This function must be called after the kernel page table is set up.
#[cfg(target_arch = "riscv64")]
pub unsafe fn switch_to_kernel_page_table() {
    // SAFETY: the boot page table never gets dropped.
    unsafe {
        switch_page_table(&KERNEL_PAGE_TABLE);
    }
}

/// Switch to the kernel page table.
///
/// # Note for LoongArch
/// In our LoongArch implementation, we do not have a separate kernel page table.
/// The mapping for the kernel space is done in a direct mapping configuration window.
/// Therefore, this function just disables the current user page table, leaving only
/// the kernel space mapped.
#[cfg(target_arch = "loongarch64")]
pub unsafe fn switch_to_kernel_page_table() {
    pgdl::set_base(0);
}

/// Switches to the specified page table.
///
/// # Safety
/// This function must be called before the current page table is dropped,
/// or the kernel may lose its memory mappings.
pub unsafe fn switch_page_table(page_table: &PageTable) {
    switch_pagetable(page_table.root().to_usize());
    log::trace!(
        "Switched to page table at {:#x}",
        page_table.root().to_usize(),
    );
}

/// Prints the lookup process of a virtual address in the specific page table.
///
/// For debugging purposes.
pub fn trace_page_table_lookup(root: PhysPageNum, va: VirtAddr) {
    let mut ppn = root;
    for (i, index) in va.page_number().indices().into_iter().enumerate().rev() {
        let page_table = unsafe { PageTableMem::new(ppn) };
        let entry = page_table.get_entry(index);
        log::error!(
            "Level {}: page table at {:#x}, entry at offset {:#x} * 64 = {:#x}, entry: {:#x?}",
            i,
            ppn.address().to_usize(),
            index,
            index * size_of::<PageTableEntry>(),
            entry
        );
        if !entry.is_valid() {
            return;
        }
        ppn = entry.ppn();
    }
}

/// Prints the lookup process of a virtual address in the kernel page table.
///
/// For debugging purposes.
#[cfg(target_arch = "riscv64")]
pub fn trace_kernel_page_table_lookup(va: VirtAddr) {
    trace_page_table_lookup(KERNEL_PAGE_TABLE.root(), va);
}
