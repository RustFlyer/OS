use alloc::sync::Arc;
use bitflags::bitflags;
use config::mm::{MMAP_END, MMAP_START, PAGE_SIZE};
use mm::address::VirtAddr;
use systype::{SysError, SysResult};
use vfs::file::File;

use crate::vm::mapping_flags::MappingFlags;

use super::{
    addr_space::AddrSpace,
    vm_area::{VmArea, VmaFlags},
};

bitflags! {
    // See in "bits/mman-linux.h"
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapFlags: i32 {
        // Sharing types (must choose one and only one of these).
        /// Share changes.
        const MAP_SHARED = 0x01;
        /// Changes are private.
        const MAP_PRIVATE = 0x02;
        /// Share changes and validate
        const MAP_SHARED_VALIDATE = 0x03;
        const MAP_TYPE_MASK = 0x03;

        // Other flags
        /// Interpret addr exactly.
        const MAP_FIXED = 0x10;
        /// Don't use a file.
        const MAP_ANONYMOUS = 0x20;
        /// Don't check for reservations.
        const MAP_NORESERVE = 0x04000;
    }
}

bitflags! {
    // See in "bits/mman-linux.h"
    // NOTE: Zero bit flag is discouraged. See https://docs.rs/bitflags/latest/bitflags/#zero-bit-flags
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapProt: i32 {
        /// No access.
        const PROT_NONE = 0x0;
        /// Page can be read.
        const PROT_READ = 0x1;
        /// Page can be written.
        const PROT_WRITE = 0x2;
        /// Page can be executed.
        const PROT_EXEC = 0x4;
    }
}

impl MappingFlags {
    /// Creates a set of `MappingFlags` from a set of `MmapProt`. `RWX` bits are set
    /// according to the `MmapProt` bits.
    pub fn from_mmapprot(prot: MmapProt) -> Self {
        let mut ret = MappingFlags::empty();
        if prot.contains(MmapProt::PROT_READ) {
            ret |= Self::R;
        }
        if prot.contains(MmapProt::PROT_WRITE) {
            ret |= Self::W;
        }
        if prot.contains(MmapProt::PROT_EXEC) {
            ret |= Self::X;
        }
        ret
    }
}

impl AddrSpace {
    /// Maps a file into the process's virtual memory space.
    ///
    /// Creates a memory-mapped region for the given file, allowing it to be accessed
    /// as if it were in memory. The mapping can be either private or shared.
    ///
    /// # Returns
    /// On success, returns the actual virtual address where the file was mapped.
    /// On failure, returns a `SysError` indicating the reason.
    ///
    /// # Attention
    /// - If `va` is 0, the system automatically finds a suitable virtual address range
    pub fn map_file(
        &self,
        file: Option<Arc<dyn File>>,
        flags: MmapFlags,
        prot: MmapProt,
        mut addr: VirtAddr,
        length: usize,
        offset: usize,
    ) -> SysResult<usize> {
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

        let length = (length + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let va_start = addr;
        let va_end = VirtAddr::new(addr.to_usize() + length);

        let vma_flags = match flags.intersection(MmapFlags::MAP_TYPE_MASK) {
            MmapFlags::MAP_PRIVATE => Ok(VmaFlags::PRIVATE),
            MmapFlags::MAP_SHARED => Ok(VmaFlags::SHARED),
            e => {
                log::error!("[map_file] invalid flag: {:?}", e);
                Err(SysError::EINVAL)
            }
        }?;

        let mem_prot = MappingFlags::from_mmapprot(prot);

        // info!("[map_file] vma_flag: [{vma_flag:?}], mem_prot: [{mem_prot:?}]");
        // info!("[map_file] offset: [{offset:?}], length: [{length:?}]");
        // info!("[map_file] va_start: [{va_start:?}], va_end: [{va_end:?}]");

        let area = match file {
            Some(file) => VmArea::new_file_backed(
                va_start,
                va_end,
                vma_flags,
                mem_prot,
                Arc::clone(&file),
                offset,
                length,
            ),
            None => VmArea::new_anonymous(va_start, va_end, vma_flags, mem_prot),
        };

        if flags.contains(MmapFlags::MAP_FIXED) {
            log::debug!(
                "[map_file] MmapFlags::MAP_FIXED remove area {:?} - {:?}",
                va_start,
                va_end
            );
            self.remove_mapping(va_start, length);
        }

        self.add_area(area)?;

        Ok(va_start.to_usize())
    }
}
