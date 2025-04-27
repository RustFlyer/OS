use config::mm::PAGE_SIZE;
use mm::address::VirtAddr;
use systype::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    vm::{
        mapping_flags::MappingFlags,
        mmap::{MmapFlags, MmapProt},
    },
};

/// `mmap()` creates a new mapping in the virtual address space of the calling process.
/// The starting address for the new mapping is specified in addr. The `length` argument
/// specifies the length of the mapping (which must be greater than 0).
///
/// # Addr
/// - If `addr` is NULL, then the kernel chooses the (page-aligned) address at which to create the mapping;
///   this is the most portable method of creating a new mapping.
/// - If `addr` is not NULL, then the kernel takes it as a hint about where to place the mapping;
///   on Linux, the kernel will pick a nearby page boundary and attempt to create the mapping there.
///   If another mapping already exists there, the kernel picks a new address that may or may not
///   depend on the hint.  The address of the new mapping is returned as the result of the call.
///
/// # Prot
/// The prot argument describes the desired memory protection of the mapping (and must not
/// conflict with the open mode of the file). It is either `PROT_NONE` or the bitwise OR of
/// one or more of the following flags:
/// - `PROT_EXEC`  Pages may be executed.
/// - `PROT_READ`  Pages may be read.
/// - `PROT_WRITE` Pages may be written.
/// - `PROT_NONE`  Pages may not be accessed.
///
/// # Flags
/// The flags argument determines whether updates to the mapping are visible to other processes
/// mapping the same region, and whether updates are carried through to the underlying file.
/// This behavior is determined by including exactly one of the following values in flags:
/// - `MAP_SHARED`: Share this mapping. Updates to the mapping are visible to other processes
///   mapping the same region.
/// - `MAP_PRIVATE`: Create a private copy-on-write mapping. Updates to the mapping are not visible
///   to other processes mapping the same file, and are not carried through to the underlying
///   file. It is unspecified whether changes made to the file after the mmap() call are visible
///   in the mapped region.
pub async fn sys_mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: isize,
    offset: usize,
) -> SyscallResult {
    let task = current_task();
    log::trace!("[sys_mmap] fd: {fd:#x}");
    let file = match fd {
        -1 => None,
        fd => Some(task.with_mut_fdtable(|table| table.get_file(fd as usize))?),
    };
    let flags = MmapFlags::from_bits_truncate(flags);
    let prot = MmapProt::from_bits_truncate(prot);
    let va = VirtAddr::new(addr);

    log::info!("[sys_mmap] addr: {addr:#x}, prot: {prot:?}, flags: {flags:?}");

    if addr == 0 && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    task.addr_space()
        .map_file(file, flags, prot, va, length, offset)
}

/// `munmap()` deletes the mappings for the specified address range, and causes further
/// references to addresses within the range to generate invalid memory references.
///
/// The region is also automatically unmapped when the process is terminated.
/// On the other hand, closing the file descriptor does not unmap the region.
///
/// The address `addr` must be a multiple of the page size (but `length` need not be).
///
/// All pages containing a part of the indicated range are unmapped, and subsequent references
/// to these pages will generate `SIGSEGV`. It is not an error if the indicated range does
/// not contain any mapped pages.
pub async fn sys_munmap(addr: usize, length: usize) -> SyscallResult {
    log::info!("[sys_munmap] addr: {addr:#x}, length: {length:#x}");

    let task = current_task();
    let addr = VirtAddr::new(addr);
    task.addr_space().remove_mapping(addr, length);
    Ok(0)
}

/// brk() and sbrk() change the location of the program break, which defines the end of the
/// process's data segment.
///
/// brk() sets the end of the data segment to the value specified by addr, when that value
/// is reasonable, the system has enough memory, and the process does not exceed its
/// maximum data size.
pub async fn sys_brk(addr: usize) -> SyscallResult {
    log::info!("[sys_brk] addr: {addr:#x}");

    current_task().addr_space().change_heap_size(addr, 0)
}

pub fn sys_mprotect(addr: usize, len: usize, prot: i32) -> SyscallResult {
    if addr == 0 || addr % PAGE_SIZE != 0 {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let addr_space = task.addr_space();
    let prot = MmapProt::from_bits(prot).ok_or(SysError::EINVAL)?;

    log::info!("[sys_mprotect] addr: {addr:#x}, len: {len:#x}, prot: {prot:?}");

    addr_space.change_prot(VirtAddr::new(addr), len, MappingFlags::from_mmapprot(prot));
    Ok(0)
}
