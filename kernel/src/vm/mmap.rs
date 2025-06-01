use alloc::sync::Arc;

use config::mm::{MMAP_END, MMAP_START, PAGE_SIZE};
use mm::address::VirtAddr;
use systype::{
    error::{SysError, SysResult},
    memory_flags::{MappingFlags, MmapFlags},
};
use vfs::file::File;

use super::{
    addr_space::AddrSpace,
    vm_area::{VmArea, VmaFlags},
};

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
        prot: MappingFlags,
        mut addr: VirtAddr,
        length: usize,
        offset: usize,
    ) -> SysResult<usize> {
        if !flags.contains(MmapFlags::MAP_FIXED) {
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

        let area = match file {
            Some(file) => VmArea::new_file_backed(
                va_start,
                va_end,
                vma_flags,
                prot,
                Arc::clone(&file),
                offset,
                length,
            ),
            None => VmArea::new_anonymous(va_start, va_end, vma_flags, prot),
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
