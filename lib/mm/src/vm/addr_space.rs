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

use core::ops::{Bound, ControlFlow};

use alloc::collections::btree_map::BTreeMap;

use systype::{SysError, SysResult};

use crate::address::VirtAddr;

use super::{
    page_table::{self, PageTable},
    vm_area::{MemPerm, PageFaultInfo, VmArea},
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
            .map(|(_, vma)| vma.end_va().round_up() > area.start_va().round_down())
            .unwrap_or(false)
        {
            return Err(SysError::EINVAL);
        }
        if lower_gap
            .peek_next()
            .map(|(&start_va, _)| start_va.round_down() < area.end_va().round_up())
            .unwrap_or(false)
        {
            return Err(SysError::EINVAL);
        }

        self.vm_areas.insert(area.start_va(), area);
        Ok(())
    }

    /// Checks if certain user memory access is allowed, given the starting address
    /// and length.
    pub fn check_user_access(
        &mut self,
        addr: VirtAddr,
        len: usize,
        perm: MemPerm,
    ) -> SysResult<()> {
        todo!()
    }

    /// Checks if certain user memory access is allowed, given the starting address,
    /// the length, and a closure which performs additional actions along with the
    /// check and controls whether to stop the process early.
    ///
    /// The closure takes a reference to a `T` value on the memory region, and it
    /// should return a [`ControlFlow<()>`] value to indicate whether to stop the
    /// process early.
    pub fn check_user_access_with<F, T>(
        &mut self,
        addr: VirtAddr,
        len: usize,
        perm: MemPerm,
        f: F,
    ) -> SysResult<()>
    where
        F: FnMut(&T) -> ControlFlow<()>,
    {
        todo!()
    }

    /// Checks if certain user memory access in a page is allowed, given the page number.
    pub fn check_user_access_page(&mut self, page_num: usize, perm: MemPerm) -> SysResult<()> {
        todo!()
    }

    /// Removes a VMA from the address space, specifying its starting virtual address.
    ///
    /// This function removes a VMA from the address space, which de facto unmaps the memory
    /// region in the address space. If there is no such VMA, this function does nothing.
    pub fn remove_area(&mut self, start_va: VirtAddr) {
        self.vm_areas.remove(&start_va);
    }

    /// Handles a page fault happened in the address space.
    ///
    /// This function is called when a page fault happens in the address space. It finds the
    /// VMA that contains the fault address and calls the VMA's page fault handler to handle
    /// the page fault.
    ///
    /// # Errors
    /// Returns [`SysError::EFAULT`] if the fault address is invalid or the access permission
    /// is not allowed.
    pub fn handle_page_fault(&mut self, fault_addr: VirtAddr, access: MemPerm) -> SysResult<()> {
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
pub fn switch_to(_old_space: &AddrSpace, new_space: &AddrSpace) {
    // SAFETY: We force the user of this function to send a reference to the old address space,
    // so the old page table is still valid.
    unsafe {
        page_table::switch_page_table(&new_space.page_table);
    }
}
