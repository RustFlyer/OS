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

use alloc::collections::btree_map::BTreeMap;

use systype::SysResult;

use crate::address::VirtAddr;

use super::{page_table::PageTable, vm_area::VmArea};

/// A virtual address space.
///
/// See the module-level documentation for more information.
#[derive(Debug)]
pub struct AddrSpace {
    /// Page table of the address space.
    pub page_table: PageTable,
    /// VMAs of the address space.
    pub vm_areas: BTreeMap<VirtAddr, VmArea>,
}

impl AddrSpace {
    /// Create an empty address space.
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

    /// Insert a VMA into the address space.
    ///
    /// This function inserts a VMA into the address space, which de facto builds a memory
    /// mapping in the address space.
    pub fn insert_vma(&mut self, vma: VmArea) {
        self.vm_areas.insert(vma.start_va, vma);
    }

    /// Remove a VMA from the address space.
    ///
    /// This function removes a VMA from the address space, which de facto unmaps the memory
    /// region in the address space.
    pub fn remove_vma(&mut self, start_va: VirtAddr) {
        self.vm_areas.remove(&start_va);
    }
}
