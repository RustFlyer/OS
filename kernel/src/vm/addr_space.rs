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

use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use core::{cmp, ops::Bound};

use arch::riscv64::mm::{fence, tlb_shootdown_all};
use mm::address::VirtAddr;
use mutex::SpinLock;
use systype::{SysError, SysResult};

use super::{
    mem_perm::MemPerm,
    page_table::{self, PageTable},
    pte::PteFlags,
    vm_area::{PageFaultInfo, VmArea},
};

/// A virtual address space.
///
/// See the module-level documentation for more information.
#[derive(Debug)]
pub struct AddrSpace {
    /// Page table of the address space.
    pub page_table: PageTable,
    /// VMAs of the address space.
    ///
    /// Note: Be careful when using this field directly.
    pub vm_areas: SpinLock<BTreeMap<VirtAddr, VmArea>>,
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
            vm_areas: SpinLock::new(BTreeMap::new()),
        })
    }

    /// Creates an empty address space with the kernel part mapped for a user process.
    ///
    /// This should be the base of the address space for any user process.
    ///
    /// # Errors
    /// Returns [`ENOMEM`] if memory allocation needed for the address space fails.
    pub fn build_user() -> SysResult<Self> {
        let addr_space = Self::build()?;
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
    pub fn add_area(&self, area: VmArea) -> SysResult<()> {
        let mut vm_areas_lock = self.vm_areas.lock();
        let lower_gap = vm_areas_lock.upper_bound(Bound::Included(&area.start_va()));
        if lower_gap
            .peek_prev()
            .map(|(_, vma)| vma.end_va() > area.start_va())
            .unwrap_or(false)
        {
            log::info!("prev overlap {:?}", area);
            return Err(SysError::EINVAL);
        }
        if lower_gap
            .peek_next()
            .map(|(&start_va, _)| start_va < area.end_va())
            .unwrap_or(false)
        {
            log::info!("next overlap {:?}", area);
            return Err(SysError::EINVAL);
        }

        vm_areas_lock.insert(area.start_va(), area);
        Ok(())
    }

    /// Finds a vacant memory region in the user address space.
    ///
    /// This function first tries to find a vacant memory region that starts from `start_va`
    /// and has a length of `length`. If such requirement cannot be satisfied, it tries to
    /// find a vacant memory region elsewhere from `find_from` to `find_to`, and the starting
    /// address of the region is guaranteed to be larger than `start_va`.
    ///
    /// The region to be found is always page-aligned.
    ///
    /// Returns the starting address of the vacant memory region if found.
    pub fn find_vacant_memory(
        &self,
        start_va: VirtAddr,
        length: usize,
        find_from: VirtAddr,
        find_to: VirtAddr,
    ) -> Option<VirtAddr> {
        let length = VirtAddr::new(length).round_up().to_usize();
        let mem_start = start_va.round_up();
        let mem_end = VirtAddr::new(mem_start.to_usize() + length);
        let vm_areas_lock = self.vm_areas.lock();

        // Check if the specified range is vacant.
        if mem_start.to_usize() >= find_from.to_usize() && mem_end.to_usize() <= find_to.to_usize()
        {
            let gap = vm_areas_lock.upper_bound(Bound::Included(&mem_start));
            let vma_prev = gap.peek_prev().map(|(_, vma)| vma);
            let vma_next = gap.peek_next().map(|(_, vma)| vma);
            let vacant_lower = vma_prev
                .map(|vma| vma.end_va() <= mem_start)
                .unwrap_or(true);
            let vacant_upper = vma_next
                .map(|vma| vma.start_va() >= mem_end)
                .unwrap_or(true);
            if vacant_lower && vacant_upper {
                return Some(mem_start);
            }
        }

        // Find a vacant region after `start_va` in the range from `find_from` to `find_to`.
        let mem_start = cmp::max(find_from, mem_start);
        let mem_end = VirtAddr::new(mem_start.to_usize() + length);
        let mut iter = vm_areas_lock
            .iter()
            .map(|(_, vma)| vma)
            .skip_while(|&vma| vma.end_va() <= mem_start)
            .filter(|&vma| vma.start_va() < find_to)
            .peekable();
        // Try the range from `mem_start` to `mem_end`.
        if iter
            .peek()
            .map(|&vma| mem_end <= cmp::min(vma.start_va(), find_to))
            .unwrap_or(true)
        {
            return Some(mem_start);
        }
        // Otherwise, try to find a vacant region after one of the VMAs.
        while let Some(vma) = iter.next() {
            let end_va = vma.end_va();
            let next_start_va = iter.peek().map(|&vma| vma.start_va()).unwrap_or(find_to);
            if next_start_va.to_usize().saturating_sub(end_va.to_usize()) >= length {
                return Some(end_va);
            }
        }

        None
    }

    /// Removes mappings for the specified address range.
    ///
    /// This function removes mappings for the specified address range.
    /// If the range is not mapped, this function does nothing. If the
    /// range covers only part of any VMA, the VMA may shrink or split.
    /// Page table entries in the range are also invalidated.
    ///
    /// `addr` must be a multiple of the page size. `length` need not to be.
    /// However, the range to be removed is rounded up to page size, which
    /// means more than `length` bytes will be removed if `length` is not
    /// page-aligned. `addr + length` should be a valid address.
    pub fn remove_mapping(&self, addr: VirtAddr, length: usize) {
        let length = VirtAddr::new(length).round_up().to_usize();
        let end_addr = VirtAddr::new(addr.to_usize() + length);
        let mut vm_areas_lock = self.vm_areas.lock();

        // Find VMAs that overlap with the specified range.
        let mut keys = vm_areas_lock
            .range(addr..end_addr)
            .map(|(&va, _)| va)
            .collect::<Vec<_>>();
        match vm_areas_lock
            .upper_bound(Bound::Excluded(&addr))
            .peek_prev()
        {
            Some((&va, vma)) if vma.end_va() > addr => {
                keys.push(va);
            }
            _ => {}
        }

        // Remove mappings for these VMAs.
        for key in keys {
            let vma = vm_areas_lock.remove(&key).unwrap();
            let (vma_low, vma_mid, vma_high) = vma.split_area(addr, end_addr);
            if let Some(vma_low) = vma_low {
                vm_areas_lock.insert(vma_low.start_va(), vma_low);
            }
            if let Some(vma_mid) = vma_mid {
                vma_mid.unmap_area(&self.page_table);
            }
            if let Some(vma_high) = vma_high {
                vm_areas_lock.insert(vma_high.start_va(), vma_high);
            }
        }
    }

    /// Changes the protection of a memory region.
    ///
    /// This function changes the protection of a memory region in the address space.
    /// The region to be changed is specified by the starting address and the length.
    /// If the region is not mapped, this function does nothing. If the region covers
    /// only part of any VMA, the VMA may split.
    ///
    /// `addr` must be a multiple of the page size. `length` need not to be. However,
    /// the range to be changed is rounded up to page size, which means more than
    /// `length` bytes will be changed if `length` is not page-aligned. `addr + length`
    /// should be a valid address.
    pub fn change_prot(&self, addr: VirtAddr, length: usize, prot: MemPerm) {
        let length = VirtAddr::new(length).round_up().to_usize();
        let end_addr = VirtAddr::new(addr.to_usize() + length);
        let mut vm_areas_lock = self.vm_areas.lock();

        // Find VMAs that overlap with the specified range.
        let mut keys = vm_areas_lock
            .range(addr..end_addr)
            .map(|(&va, _)| va)
            .collect::<Vec<_>>();
        match vm_areas_lock
            .upper_bound(Bound::Excluded(&addr))
            .peek_prev()
        {
            Some((&va, vma)) if vma.end_va() > addr => {
                keys.push(va);
            }
            _ => {}
        }

        // Change protection for these VMAs.
        for key in keys {
            let vma = vm_areas_lock.remove(&key).unwrap();
            // let (vma1, vma2) = vma.change_prot(&self.page_table, addr, end_addr, prot);
            let (vma_low, vma_mid, vma_high) = vma.split_area(addr, end_addr);
            if let Some(vma_low) = vma_low {
                vm_areas_lock.insert(vma_low.start_va(), vma_low);
            }
            if let Some(mut vma_mid) = vma_mid {
                vma_mid.change_prot(&self.page_table, prot);
                vm_areas_lock.insert(vma_mid.start_va(), vma_mid);
            }
            if let Some(vma_high) = vma_high {
                vm_areas_lock.insert(vma_high.start_va(), vma_high);
            }
        }
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
    pub fn clone_cow(&self) -> SysResult<Self> {
        let mut new_space = Self::build_user()?;
        let new_vm_areas = self.vm_areas.lock().clone();

        for &vpn in new_vm_areas.values().flat_map(|vma| vma.pages().keys()) {
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
        new_space.vm_areas = SpinLock::new(new_vm_areas);

        // Because the permission of PTEs is downgraded, we need to do a TLB shootdown.
        fence();
        tlb_shootdown_all();

        Ok(new_space)
    }

    /// Changes the size of the heap.
    ///
    /// This function changes the size of the heap by changing the end of the heap area.
    ///
    /// In order to implement `brk` and `sbrk` at the same time, this function takes an address
    /// and an increment value, and only one of them is used. Unused value should be set to 0.
    /// If both of them are set to 0, this function do as if `sbrk(0)` is called.
    ///
    /// # Errors
    /// Returns [`SysError::ENOMEM`] if it is impossible to change the heap size as specified.
    pub fn change_heap_size(&self, mut addr: usize, incr: isize) -> SysResult<usize> {
        if addr != 0 && (!VirtAddr::check_validity(addr) || !VirtAddr::new(addr).in_user_space()) {
            return Err(SysError::ENOMEM);
        }

        // Find the heap area
        let mut vm_areas_lock = self.vm_areas.lock();
        let mut vma_iter = vm_areas_lock.iter_mut();
        let heap_area = vma_iter.find(|(_, vma)| vma.is_heap()).unwrap().1;
        let heap_start = heap_area.start_va().to_usize();
        let heap_end = heap_area.end_va().to_usize();

        // Calculate the new heap end
        if addr == 0 {
            addr = heap_end.checked_add_signed(incr).ok_or(SysError::ENOMEM)?;
        }

        // Check if the new heap end is valid
        if addr < heap_start
            || !VirtAddr::check_validity(addr)
            || !VirtAddr::new(addr).in_user_space()
        {
            return Err(SysError::ENOMEM);
        }
        let next_vma_start = vma_iter.next().map(|(va, _)| va.to_usize());
        match next_vma_start {
            Some(next_start) if addr > next_start => Err(SysError::ENOMEM),
            _ => {
                unsafe {
                    heap_area.set_end_va(VirtAddr::new(addr));
                }
                Ok(addr)
            }
        }
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
    pub fn handle_page_fault(&self, fault_addr: VirtAddr, access: MemPerm) -> SysResult<()> {
        let mut vm_areas_lock = self.vm_areas.lock();

        if fault_addr.to_usize() == 0x68094 {
            vm_areas_lock.iter().for_each(|(_, vma)| {
                log::debug!(
                    "[{:?}][{:?}]: {:#x} ~ {:#x}",
                    vma.map_type,
                    vma.flags(),
                    vma.start_va().to_usize(),
                    vma.end_va().to_usize()
                )
            });
        }

        let vma = vm_areas_lock
            .range_mut(..=fault_addr)
            .next_back()
            .filter(|(_, vma)| vma.contains(fault_addr))
            .map(|(_, vma)| vma)
            .ok_or(SysError::EFAULT)?;

        let page_fault_info = PageFaultInfo {
            fault_addr,
            page_table: &self.page_table,
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
