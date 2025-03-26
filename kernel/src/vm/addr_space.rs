//! Module for managing virtual address spaces.
//!
//! An address space is a mapping from virtual address space to physical address space,
//! managed by a [`AddrSpace`] struct. An address space is divided into two parts: the
//! user part and the kernel part. The user part of an address space consists of a set
//! of virtual memory areas (VMAs), represented by [`VmArea`] structs. The kernel
//! part of an address space is not managed by [`VmArea`]; rather, it is mapped in the
//! address space directly when a process is created.
//!
//! Besides VMAs, an address space also contains a page table, represented by a
//! [`PageTable`] struct. Based on the collaboration between VMAs and the page table,
//! this module provides a set of functions to manage the address space for the upper
//! layer, such as creating an address space for a process, mapping a memory region
//! into the address space, handling page faults, and so on.
//!
//! When a process is created, the kernel creates a new address space for the process.
//! The kernel creates a new page table for the address space and maps its kernel part
//! directly. VMAs are then created to manage the user part of the address space.

use core::ops::Bound;

use alloc::collections::btree_map::BTreeMap;

use arch::riscv64::mm::{fence, tlb_shootdown_all};
use config::mm::{USER_END, USER_START};
use mm::address::VirtAddr;
use systype::{SysError, SysResult};

use crate::vm::pte::PteFlags;

use super::{
    mem_perm::MemPerm,
    page_table::{self, PageTable},
    vm_area::{PageFaultInfo, VmArea},
};

/// A virtual address space.
///
/// See the module-level documentation for more information.
#[derive(Debug)]
pub struct AddrSpace {
    /// Page table of the address space.
    page_table: PageTable,
    /// VMAs of the address space.
    vm_areas: BTreeMap<VirtAddr, VmArea>,
}

impl AddrSpace {
    /// Creates an empty address space.
    ///
    /// This function is private because normally there is no need to create an address
    /// space that is completely empty. Use [`build_user`] to create an address space
    /// with the kernel part mapped.
    ///
    /// # Errors
    /// Returns [`ENOMEM`] if memory allocation needed for the address space fails.
    fn build() -> SysResult<Self> {
        Ok(Self {
            page_table: PageTable::build()?,
            vm_areas: BTreeMap::new(),
        })
    }

    /// Creates an empty address space with the kernel part mapped for a user process.
    ///
    /// This should be the base of the address space for any user process.
    ///
    /// # Errors
    /// Returns [`ENOMEM`] if memory allocation needed for the address space fails.
    pub fn build_user() -> SysResult<Self> {
        let mut addr_space = Self::build()?;
        addr_space.page_table.map_kernel();
        Ok(addr_space)
    }

    /// Adds a VMA into the address space.
    ///
    /// This function adds a VMA into the address space, which de facto builds a memory
    /// mapping in the address space. The VMA to be added must not overlap with any existing
    /// VMA in the address space; “overlapping” means that the two VMAs have any common
    /// pages, not just the starting or ending address.
    ///
    /// # Errors
    /// Returns [`SysError::EINVAL`] if the VMA to be added overlaps with any existing VMA.
    pub fn add_area(&mut self, area: VmArea) -> SysResult<()> {
        let lower_gap = self.vm_areas.upper_bound(Bound::Included(&area.start_va()));
        if lower_gap
            .peek_prev()
            .map(|(_, vma)| vma.end_va() > area.start_va())
            .unwrap_or(false)
        {
            return Err(SysError::EINVAL);
        }
        if lower_gap
            .peek_next()
            .map(|(&start_va, _)| start_va < area.end_va())
            .unwrap_or(false)
        {
            return Err(SysError::EINVAL);
        }

        self.vm_areas.insert(area.start_va(), area);
        Ok(())
    }

    /// Finds a vacant memory region in the user part of the address space.
    ///
    /// This function first tries to find a vacant memory region that starts from `start_va`
    /// and has a length of `length`. If such requirement cannot be satisfied, it tries to
    /// find a vacant memory region that has a length of `length` and starts from any address.
    ///
    /// `start_va` must be page-aligned. `length` need not to be. However, the region to be
    /// found is rounded up to the page size.
    ///
    /// Returns the starting address of the vacant memory region if found.
    pub fn find_vacant_memory(&self, start_va: VirtAddr, length: usize) -> Option<VirtAddr> {
        let length = VirtAddr::new(length).round_up().to_usize();

        // Check if the specified range is vacant.
        let gap = self.vm_areas.upper_bound(Bound::Included(&start_va));
        let vma_prev = gap.peek_prev().map(|(_, vma)| vma);
        let vma_next = gap.peek_next().map(|(_, vma)| vma);
        if vma_prev.map(|vma| vma.end_va() <= start_va).unwrap_or(true)
            && vma_next
                .map(|vma| vma.start_va() >= VirtAddr::new(start_va.to_usize() + length))
                .unwrap_or(true)
        {
            return Some(start_va);
        }

        // Find a vacant region elsewhere.

        // If there are more than two VMAs, try to find a gap between two VMAs.
        let mut iter = self.vm_areas.iter().peekable();
        while let Some((&_, vma)) = iter.next() {
            let end_va = vma.end_va();
            if let Some(&(&next_start_va, _)) = iter.peek() {
                if next_start_va.to_usize() - end_va.to_usize() >= length {
                    return Some(end_va);
                }
            }
        }

        // Look at the regions before the first VMA and after the last VMA.
        if let Some((_, first_vma)) = self.vm_areas.iter().next() {
            if first_vma.start_va().to_usize() - USER_START >= length {
                return Some(VirtAddr::new(USER_START));
            }
            let (_, last_vma) = self.vm_areas.iter().next_back().unwrap();
            if USER_END - last_vma.end_va().to_usize() >= length {
                return Some(last_vma.end_va());
            }
        } else {
            // If there is no VMA, the whole user part is vacant.
            return Some(VirtAddr::new(USER_START));
        }

        None
    }

    /// Removes mappings for the specified address range.
    ///
    /// This function removes mappings for the specified address range, and causes
    /// further references to addresses within the range to generate invalid memory
    /// references. If the range is not mapped, this function does nothing. If the
    /// range covers only part of any VMA, the VMA may shrink or split.
    ///
    /// `addr` must be a multiple of the page size. `length` need not to be. However,
    /// the range to be removed is rounded up to the page size.
    pub fn remove_mapping(&mut self, addr: VirtAddr, length: usize) {
        // Align `length` to the page size.
        let length = VirtAddr::new(length).round_up().to_usize();
        // Find all VMAs that overlap with the range.
        while let Some(_vma) = {
            let gap = self
                .vm_areas
                .upper_bound(Bound::Excluded(&VirtAddr::new(addr.to_usize() + length)));
            if let Some((&va, vma)) = gap.peek_prev() {
                if vma.end_va() > addr {
                    Some(self.vm_areas.remove(&va).unwrap())
                } else {
                    None
                }
            } else {
                None
            }
        } {}
        unimplemented!();
    }

    /// Clones the address space.
    ///
    /// This function creates a new address space with the same mappings as the original
    /// address space. Specifically, the new address space maps virtual memory areas to
    /// data identical to the original address space when the function is called.
    ///
    /// This function uses the copy-on-write (COW) mechanism to share the same physical
    /// memory pages between the original address space and the new address space. When
    /// one of them writes to a shared page, the page is copied and the writer gets a
    /// new physical page elsewhere.
    pub fn clone_cow(&mut self) -> SysResult<Self> {
        let mut new_space = Self::build_user()?;

        new_space.vm_areas = self.vm_areas.clone();

        for &vpn in self.vm_areas.iter().flat_map(|(_, vma)| vma.pages().keys()) {
            let old_pte = self.page_table.find_entry(vpn).unwrap();
            let new_pte = new_space
                .page_table
                .find_entry_force(vpn, old_pte.flags())?
                .0;
            let mut pte = *old_pte;
            if pte.flags().contains(PteFlags::W) {
                pte.set_flags(pte.flags().difference(PteFlags::W));
                *old_pte = pte;
            }
            *new_pte = pte;
        }
        // Because the permission of PTEs is downgraded, we need to do a TLB shootdown.
        fence();
        tlb_shootdown_all();

        Ok(new_space)
    }

    /// Handles a page fault happened in the address space.
    ///
    /// This function is called when a page fault happens in the address space. It finds the
    /// VMA that contains the fault address and calls the VMA's page fault handler to handle
    /// the page fault.
    ///
    /// # Errors
    /// Returns [`SysError::EFAULT`] if the fault address is invalid or the access permission
    /// is not allowed. Otherwise, returns [`SysError::ENOMEM`] if memory allocation fails
    /// when handling the page fault.
    pub fn handle_page_fault(&mut self, fault_addr: VirtAddr, access: MemPerm) -> SysResult<()> {
        simdebug::when_debug!({
            log::trace!(
                "Page fault when accessing {:#x}, type: {:?}",
                fault_addr.to_usize(),
                access
            );
        });
        let page_table = &mut self.page_table;
        let vma = self
            .vm_areas
            .range_mut(..=fault_addr)
            .next_back()
            .filter(|(_, vma)| vma.contains(fault_addr))
            .map(|(_, vma)| vma)
            .ok_or(SysError::EFAULT)?;

        let page_fault_info = PageFaultInfo {
            fault_addr,
            page_table,
            access,
        };
        vma.handle_page_fault(page_fault_info)
    }
}

/// Switches to a new address space.
///
/// This function switches the current address space to a new address space. It is used
/// when a process is scheduled in or out.
///
/// # Safety
/// This function must be called before the current page table is dropped, or the kernel
/// may lose its memory mappings.
pub unsafe fn switch_to(new_space: &AddrSpace) {
    unsafe {
        page_table::switch_page_table(&new_space.page_table);
    }
}

pub fn test_find_vacant_memory() {
    let mut addr_space = AddrSpace::build_user().unwrap();

    static MEMORY_1: &[u8] = &[0u8; 0x2000];
    static MEMORY_2: &[u8] = &[1u8; 0x3000];
    static MEMORY_3: &[u8] = &[2u8; 0x4000];
    static MEMORY_4: &[u8] = &[3u8; 0x2000];

    let area1 = VmArea::new_memory_backed(
        VirtAddr::new(0x1000),
        VirtAddr::new(0x3000),
        PteFlags::R | PteFlags::W,
        MEMORY_1,
    );
    let area2 = VmArea::new_memory_backed(
        VirtAddr::new(0x4000),
        VirtAddr::new(0x7000),
        PteFlags::R | PteFlags::W,
        MEMORY_2,
    );
    let area3 = VmArea::new_memory_backed(
        VirtAddr::new(0xa000),
        VirtAddr::new(0xe000),
        PteFlags::R | PteFlags::W,
        MEMORY_3,
    );

    addr_space.add_area(area1).unwrap();
    // These assertions should be suitable for the current implementation,
    // but they are not necessarily true for the purpose of the function.
    assert_eq!(
        addr_space.find_vacant_memory(VirtAddr::new(0x0000), 0x1000),
        Some(VirtAddr::new(0x0000))
    );
    assert_eq!(
        addr_space.find_vacant_memory(VirtAddr::new(0x1000), 0x3000),
        Some(VirtAddr::new(0x3000))
    );

    addr_space.add_area(area2).unwrap();
    addr_space.add_area(area3).unwrap();
    if let Some(addr) = addr_space.find_vacant_memory(VirtAddr::new(0x0000), 0x2000) {
        assert_eq!(addr.to_usize(), 0x7000);
        let area4 = VmArea::new_memory_backed(
            addr,
            VirtAddr::new(addr.to_usize() + 0x2000),
            PteFlags::R | PteFlags::W,
            MEMORY_4,
        );
        addr_space.add_area(area4).unwrap();
    }

    log::debug!("{:?}", addr_space.vm_areas);
}

pub fn test_clone_cow() {
    let mut old_space = AddrSpace::build_user().unwrap();

    static MEMORY_1: &[u8] = &[0u8; 0x2000];
    static MEMORY_2: &[u8] = &[1u8; 0x3000];

    let area1 = VmArea::new_memory_backed(
        VirtAddr::new(0x1000),
        VirtAddr::new(0x3000),
        PteFlags::R | PteFlags::W,
        MEMORY_1,
    );
    let area2 = VmArea::new_memory_backed(
        VirtAddr::new(0x4000),
        VirtAddr::new(0x7000),
        PteFlags::R | PteFlags::W,
        MEMORY_2,
    );

    old_space.add_area(area1).unwrap();
    old_space.add_area(area2).unwrap();

    // Manually handle a page fault to make the page mapped in the page table.
    old_space
        .handle_page_fault(VirtAddr::new(0x2100), MemPerm::W)
        .unwrap();

    let mut new_space = old_space.clone_cow().unwrap();
    // Now the two address spaces share the same physical page, which is marked as read-only.
    let old_pte = old_space
        .page_table
        .find_entry(VirtAddr::new(0x2100).page_number())
        .unwrap();
    let new_pte = new_space
        .page_table
        .find_entry(VirtAddr::new(0x2100).page_number())
        .unwrap();
    assert!(old_pte.flags().contains(PteFlags::R));
    assert!(new_pte.flags().contains(PteFlags::R));
    assert!(!old_pte.flags().contains(PteFlags::W));
    assert!(!new_pte.flags().contains(PteFlags::W));
    let old_ppn = old_pte.ppn();
    assert_eq!(new_pte.ppn(), old_ppn);

    // When the page is written, the write gets a newly copied physical page, and the PTE flags
    // are changed to writable.
    old_space
        .handle_page_fault(VirtAddr::new(0x2100), MemPerm::W)
        .unwrap();
    assert!(
        old_space
            .page_table
            .find_entry(VirtAddr::new(0x2100).page_number())
            .unwrap()
            .flags()
            .contains(PteFlags::R | PteFlags::W)
    );
    assert_ne!(
        old_space
            .page_table
            .find_entry(VirtAddr::new(0x2100).page_number())
            .unwrap()
            .ppn(),
        old_ppn
    );

    new_space
        .handle_page_fault(VirtAddr::new(0x2100), MemPerm::W)
        .unwrap();
    assert!(
        new_space
            .page_table
            .find_entry(VirtAddr::new(0x2100).page_number())
            .unwrap()
            .flags()
            .contains(PteFlags::R | PteFlags::W)
    );
    // Here, because `new_space` is the only one that owns the page, the page is not copied.
    assert_eq!(
        new_space
            .page_table
            .find_entry(VirtAddr::new(0x2100).page_number())
            .unwrap()
            .ppn(),
        old_ppn
    );
}
