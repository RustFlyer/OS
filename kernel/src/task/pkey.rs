use alloc::collections::btree_map::BTreeMap;
use id_allocator::{IdAllocator, VecIdAllocator};
use mm::address::VirtAddr;
use systype::{
    error::{SysError, SysResult},
    memory_flags::MappingFlags,
};

use super::Task;

const PKEY_DISABLE_ACCESS: u32 = 0x1; // Disable all access
const PKEY_DISABLE_WRITE: u32 = 0x2; // Disable write access

#[derive(Debug)]
pub struct PKeyTable {
    count: VecIdAllocator,
    records: BTreeMap<usize, u32>,
}

impl PKeyTable {
    pub fn new() -> Self {
        Self {
            count: VecIdAllocator::new(1, 16),
            records: BTreeMap::new(),
        }
    }

    pub fn alloc_id(&mut self) -> Option<usize> {
        self.count.alloc()
    }

    pub fn insert_pkey(&mut self, id: usize, flags: u32) {
        self.records.insert(id, flags);
    }

    pub fn free_id(&mut self, id: usize) -> bool {
        if self.records.remove(&id).is_some() {
            unsafe { self.count.dealloc(id) };
            true
        } else {
            false
        }
    }

    pub fn contains_pkey(&self, id: usize) -> bool {
        self.records.contains_key(&id)
    }

    pub fn get_pkey_flags(&self, id: usize) -> Option<u32> {
        self.records.get(&id).copied()
    }
}

impl Task {
    /// Allocate a protection key
    pub fn alloc_pkey(&self, flags: u32) -> SysResult<u32> {
        let lock = self.pkeytable_mut();
        let mut pkeytable = lock.lock();

        if let Some(id) = pkeytable.alloc_id() {
            pkeytable.insert_pkey(id, flags);
            return Ok(id as u32);
        }

        return Err(SysError::ENOSPC);
    }

    /// Free a protection key
    pub fn free_pkey(&self, pkey: u32) -> SysResult<()> {
        if pkey == 0 {
            return Err(SysError::EINVAL);
        }

        let lock = self.pkeytable_mut();
        let mut pkeytable = lock.lock();

        if !pkeytable.free_id(pkey as usize) {
            return Err(SysError::EINVAL);
        }

        Ok(())
    }

    /// Verify that a protection key is valid and allocated
    pub fn verify_pkey(&self, pkey: u32) -> SysResult<()> {
        if pkey == 0 {
            return Ok(());
        }

        let lock = self.pkeytable_mut();
        let pkeytable = lock.lock();

        if !pkeytable.contains_pkey(pkey as usize) {
            return Err(SysError::EINVAL);
        }

        Ok(())
    }

    /// Change the protection flags of a memory region, optionally associating it with a protection key (pkey).
    ///
    /// This function updates the memory region [`addr`, `addr + len`) with the given protection flags and associates the region with the specified `pkey`.
    /// In a full MPK (Memory Protection Key) implementation, this would set the pkey value in each page table entry (PTE) for the region,
    /// allowing fine-grained hardware-enforced access control in combination with per-thread PKRU registers.
    ///
    /// In this implementation:
    /// - If `pkey` is non-zero, the page permissions are additionally filtered by the access rights associated with the pkey (e.g. `PKEY_DISABLE_ACCESS`, `PKEY_DISABLE_WRITE`).
    ///   The final effective flags are computed accordingly before being applied.
    /// - If `pkey` is zero, the function restores the protection flags for the region without any pkey-based restrictions, effectively removing the pkey from the region.
    ///
    /// Note: This implementation does **not** update the actual pkey field in the page table entry; it only simulates pkey-based access control by altering the standard mapping flags.
    /// For a real MPK system, you must ensure each PTE records the associated pkey, and apply hardware PKRU logic on access.
    pub fn change_prot_with_pkey(
        &self,
        addr: VirtAddr,
        len: usize,
        mut flags: MappingFlags,
        pkey: u32,
    ) -> SysResult<()> {
        self.verify_pkey(pkey)?;

        todo!();

        // fake implement
        if let Some(accessright) = self.pkeytable_mut().lock().get_pkey_flags(pkey as usize) {
            log::error!("[change_prot_with_pkey] before flags: {:?}", flags);
            if accessright & PKEY_DISABLE_ACCESS != 0 {
                flags = MappingFlags::empty();
            } else if accessright & PKEY_DISABLE_WRITE != 0 {
                flags.remove(MappingFlags::W);
            }
        }

        log::error!(
            "[change_prot_with_pkey] addr: {addr:?}, len:{len:#x} flags: {:?}",
            flags
        );

        let addr_space = self.addr_space();
        addr_space.change_prot(addr, len, flags);

        Ok(())
    }
}
