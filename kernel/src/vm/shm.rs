use alloc::sync::Arc;

use config::mm::{MMAP_END, MMAP_START, PAGE_SIZE};
use mm::address::VirtAddr;
use mutex::ShareMutex;
use shm::SharedMemory;
use systype::{SysError, SysResult};

use super::{addr_space::AddrSpace, mmap::MmapProt};
use crate::vm::{
    mapping_flags::MappingFlags,
    vm_area::{VmArea, VmaFlags},
};

impl AddrSpace {
    /// Attach a shared memory area to the address space.
    ///
    /// `addr` is the starting address of the shared memory area. If `addr` is
    /// 0, the kernel will find a suitable address for the shared memory area.
    /// Otherwise, the kernel will try to use the specified address which must
    /// be page aligned.
    ///
    /// `length` is the length of the shared memory area. If it is not page
    /// aligned, it will be rounded up to a multiple of the page size.
    ///
    /// `shm` is the shared memory object to be attached.
    ///
    /// `prot` is the memory protection flags for the shared memory area.
    pub fn attach_shm(
        &self,
        mut addr: VirtAddr,
        length: usize,
        shm: ShareMutex<SharedMemory>,
        prot: MmapProt,
    ) -> SysResult<VirtAddr> {
        if addr.to_usize() == 0 {
            addr = self
                .find_vacant_memory(
                    addr,
                    length,
                    VirtAddr::new(MMAP_START),
                    VirtAddr::new(MMAP_END),
                )
                .ok_or(SysError::ENOMEM)?;
        }

        log::info!(
            "[attach_shm] addr: {:#x}, length: {}, prot: {:?}",
            addr.to_usize(),
            length,
            prot
        );

        let length = (length + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let va_start = addr;
        let va_end = VirtAddr::new(addr.to_usize() + length);
        let vma_flags = VmaFlags::SHARED;

        let prot = MappingFlags::from_mmapprot(prot);

        let area = VmArea::new_shared_memory(va_start, va_end, vma_flags, prot, shm);
        self.add_area(area)?;

        Ok(va_start)
    }

    /// `shmaddr` must be the return value of shmget (i.e. `shmaddr` is page
    /// aligned and in the beginning of the vm_area with type Shm). The
    /// check should be done at the caller who call `detach_shm`.
    pub fn detach_shm(self: Arc<Self>, addr: VirtAddr) -> SysResult<()> {
        let vm_areas_lock = self.vm_areas.lock();
        let area = vm_areas_lock.get(&addr).ok_or_else(|| {
            log::warn!(
                "[detach_shm] addr: no area starting at {:#x}",
                addr.to_usize()
            );
            SysError::EINVAL
        })?;

        if area.is_shared_memory() {
            let length = area.length();
            self.remove_mapping(addr, length);
            Ok(())
        } else {
            log::warn!(
                "[detach_shm] addr: no shared memory area starting at {:#x}",
                addr.to_usize()
            );
            Err(SysError::EINVAL)
        }
    }
}
