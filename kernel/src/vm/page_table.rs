//! Module for manipulating page tables and manage memory mappings.
//!
//! This module provides a `PageTable` struct that manipulates page tables in momery
//! and tracking allocated pages.

use alloc::vec::Vec;
use riscv::register::satp::{self, Satp};

use lazy_static::lazy_static;

use config::mm::{
    PTE_PER_TABLE, VIRT_END, bss_end, bss_start, data_end, data_start, kernel_end, kernel_end_phys,
    kernel_start, rodata_end, rodata_start, text_end, text_start,
};
use mm::address::{PhysPageNum, VirtAddr, VirtPageNum};
use systype::SysResult;

use crate::{
    frame::FrameTracker,
    vm::vm_area::{KernelArea, VmArea},
};

use super::pte::{PageTableEntry, PteFlags};

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
    frames: Vec<FrameTracker>,
}

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
            PageTableMem::new(root_frame.as_ppn()).clear();
        }
        Ok(PageTable {
            root: root_frame.as_ppn(),
            frames: vec![root_frame],
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
    unsafe fn build_kernel_page_table() -> Self {
        let mut page_table = Self::build().expect("out of memory");

        /* Map the kernel's .text, .rodata, .data, and .bss sections */

        let text_start_va = VirtAddr::new(text_start());
        let text_end_va = VirtAddr::new(text_end());
        let text_flags = PteFlags::R | PteFlags::X | PteFlags::G;
        let text_vma = VmArea::new_kernel(text_start_va, text_end_va, text_flags);
        KernelArea::map(&text_vma, &mut page_table);

        let rodata_start_va = VirtAddr::new(rodata_start());
        let rodata_end_va = VirtAddr::new(rodata_end());
        let rodata_flags = PteFlags::R | PteFlags::G;
        let rodata_vma = VmArea::new_kernel(rodata_start_va, rodata_end_va, rodata_flags);
        KernelArea::map(&rodata_vma, &mut page_table);

        let data_start_va = VirtAddr::new(data_start());
        let data_end_va = VirtAddr::new(data_end());
        let data_flags = PteFlags::R | PteFlags::W | PteFlags::G;
        let data_vma = VmArea::new_kernel(data_start_va, data_end_va, data_flags);
        KernelArea::map(&data_vma, &mut page_table);

        let bss_start_va = VirtAddr::new(bss_start());
        let bss_end_va = VirtAddr::new(bss_end());
        let bss_flags = PteFlags::R | PteFlags::W | PteFlags::G;
        let bss_vma = VmArea::new_kernel(bss_start_va, bss_end_va, bss_flags);
        KernelArea::map(&bss_vma, &mut page_table);

        /* Map the allocatable frames */
        let alloc_start_va = VirtAddr::new(kernel_end());
        let alloc_end_va = VirtAddr::new(VIRT_END);
        let alloc_flags = PteFlags::R | PteFlags::W | PteFlags::G;
        let alloc_vma = VmArea::new_kernel(alloc_start_va, alloc_end_va, alloc_flags);
        KernelArea::map(&alloc_vma, &mut page_table);

        page_table
    }

    /// Gets the physical page number of the root page table.
    pub fn root(&self) -> PhysPageNum {
        self.root
    }

    /// Adds a `FrameTracker` to the page table so that the frame can be deallocated
    /// when the `PageTable` is dropped. Any page table frame exclusive to the page
    /// table must be tracked by calling this method.
    pub fn track_frame(&mut self, frame: FrameTracker) {
        self.frames.push(frame);
    }

    /// Returns a mutable reference to a leaf page table entry mapping a given VPN.
    /// This method creates absent non-leaf entries using `inner_flags`. Note that
    /// the returned entry may be invalid.
    ///
    /// `inner_flags` are the flags for newly created non-leaf entries, if any.
    /// Several bits in the parameter can be set, but only bit `G` of it is used,
    /// as the non-leaf entries can only have bits `VG` set, and `V` is mandatory.
    ///
    /// This function only support 4 KiB pages, 3-level page tables.
    pub fn find_entry_create(
        &mut self,
        vpn: VirtPageNum,
        inner_flags: PteFlags,
    ) -> &mut PageTableEntry {
        let mut ppn = self.root;
        let inner_flags = PteFlags::V | (inner_flags & PteFlags::G);
        for (i, index) in vpn.indices().into_iter().enumerate().rev() {
            let mut page_table = unsafe { PageTableMem::new(ppn) };
            let entry = page_table.get_entry_mut(index);
            if i == 0 {
                return entry;
            }
            if !entry.is_valid() {
                let frame = FrameTracker::build().expect("out of memory");
                *entry = PageTableEntry::new(frame.as_ppn(), inner_flags);
                self.track_frame(frame);
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
    pub fn find_entry(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
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

    /// Maps a leaf page by specifying VPN, PPN, and page table entry flags.
    ///
    /// This method does not allocate the frame for the leaf page. It only sets the
    /// mapping in the page table. The caller should allocate a frame is allocated
    /// and set the mapping by calling this method. Be careful that calling this
    /// method with an already mapped `vpn` will overwrite the existing mapping.
    ///
    /// # Note
    /// This function takes `flags` as the flags for the leaf page table entry, and
    /// it takes bits `G` to set the flags for intermediate entries. This design
    /// provides a lower granularity of control over intermediate entries, but it is
    /// sufficient for the current address space layout.
    pub fn map_page(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PteFlags) {
        let entry = self.find_entry_create(vpn, flags);
        *entry = PageTableEntry::new(ppn, flags);
    }

    /// Unmaps a leaf page by specifying the VPN.
    ///
    /// This method does not deallocate the frame for the leaf page. It only clears the
    /// mapping in the page table. The caller should deallocate the frame and clear
    /// the mapping by calling this method. Calling this method to clear an unmapped
    /// page is safe.
    pub fn unmap_page(&mut self, vpn: VirtPageNum) {
        if let Some(entry) = self.find_entry(vpn) {
            *entry = PageTableEntry::default();
        }
    }

    /// Maps a range of leaf pages by specifying the starting VPN, a slice of PPNs,
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
    pub fn map_range(&mut self, start_vpn: VirtPageNum, ppns: &[PhysPageNum], flags: PteFlags) {
        // Optimization is applied to cut down redundant lookups to entries in the
        // same leaf page table.
        let mut entry = self.find_entry_create(start_vpn, flags);
        *entry = PageTableEntry::new(ppns[0], flags);
        for (i, &ppn) in ppns.iter().enumerate().skip(1) {
            let vpn = start_vpn.to_usize() + i;
            entry = if vpn % PTE_PER_TABLE == 0 {
                self.find_entry_create(VirtPageNum::new(vpn), flags)
            } else {
                // SAFETY: the entry is not the last one in its page table, so the
                // next entry of `entry` is valid.
                unsafe { &mut *(entry as *mut PageTableEntry).add(1) }
            };
            *entry = PageTableEntry::new(ppn, flags);
        }
    }

    /// Unmaps a range of leaf pages by specifying the starting VPN and the number of pages.
    ///
    /// The range is `[start_vpn, start_vpn + count)`.
    ///
    /// This method does not deallocate the frames for the leaf pages. It only clears the
    /// mappings in the page table. The caller should deallocate the frames and clear the
    /// mappings by calling this method. Calling this method to clear any unmapped range
    /// is safe.
    pub fn unmap_range(&mut self, start_vpn: VirtPageNum, count: usize) {
        for i in 0..count {
            let vpn = VirtPageNum::new(start_vpn.to_usize() + i);
            self.unmap_page(vpn);
        }
    }

    /// Maps the kernel part of the address space into this page table.
    ///
    /// This method is used to map the kernel space into a new page table for a user process.
    /// This method does not allocate any frame or make this page table own any frame.
    pub fn map_kernel(&mut self) {
        let kernel_vpn_start = VirtAddr::new(kernel_start()).page_number();
        let kernel_vpn_end = VirtAddr::new(kernel_end()).round_up().page_number();
        // Range of the top-level PTEs that map the kernel space.
        let index_start = kernel_vpn_start.indices()[2];
        let index_end = kernel_vpn_end.indices()[2];

        let mut page_table = unsafe { PageTableMem::new(self.root) };
        let kernel_page_table = unsafe { PageTableMem::new(KERNEL_PAGE_TABLE.root) };
        let src = &kernel_page_table.as_slice()[index_start..=index_end];
        let dst = &mut page_table.as_slice_mut()[index_start..=index_end];
        dst.copy_from_slice(src);
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
    /// Reference to the page table in memory.
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
        // unsafe { &mut *(self.ppn.to_vpn_kernel().as_slice().as_mut_ptr() as *mut _) }
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

/// Enables the kernel page table.
///
/// # Safety
/// This function must be called after the kernel page table is set up.
pub unsafe fn enable_kernel_page_table() {
    // SAFETY: the boot page table never gets dropped.
    unsafe {
        switch_page_table(&KERNEL_PAGE_TABLE);
    }
}

/// Switches to the specified page table.
///
/// # Safety
/// This function must be called before the current page table is dropped,
/// or the kernel may lose its memory mappings.
pub unsafe fn switch_page_table(page_table: &PageTable) {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(satp::Mode::Sv39);
    satp.set_ppn(page_table.root().to_usize());
    unsafe {
        satp::write(satp);
    }
    riscv::asm::sfence_vma_all();
    simdebug::when_debug!({
        log::info!(
            "Switched to page table at {:#x}, satp: {:#x}",
            page_table.root().to_usize(),
            satp::read().bits()
        );
    });
}

/// Prints page table entries of each level to resolve a given virtual address.
pub fn print_page_table_entries(page_table: &PageTable, va: VirtAddr) {
    let mut ppn = page_table.root();
    for (i, index) in va.page_number().indices().into_iter().enumerate().rev() {
        let page_table = unsafe { PageTableMem::new(ppn) };
        let entry = page_table.get_entry(index);
        log::debug!(
            "Level {}: page table at {:#x}, entry at offset {:#x}, entry: {:#x?}",
            i,
            ppn.address().to_usize(),
            index,
            entry
        );
        if !entry.is_valid() {
            return;
        }
        ppn = entry.ppn();
    }
}
