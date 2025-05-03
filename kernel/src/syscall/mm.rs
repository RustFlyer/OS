use config::mm::PAGE_SIZE;
use mm::address::VirtAddr;
use systype::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    vm::{
        mem_perm::MemPerm,
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
/// - `MAP_FIXED`: Don't interpret addr as a hint: place the mapping at exactly that address.
///   addr must be suitably aligned: for most architectures a multiple of the page size is sufficient;
///   however, some architectures may impose additional restrictions. If the memory region specified by
///   addr and length overlaps pages of any existing mapping(s), then the overlapped part of the existing
///   mapping(s) will be discarded. If the specified address cannot be used, mmap() will fail.
pub async fn sys_mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: isize,
    offset: usize,
) -> SyscallResult {
    let task = current_task();
    let flags = MmapFlags::from_bits_truncate(flags);
    let prot = MmapProt::from_bits_truncate(prot);
    let perm = MemPerm::from_mmapprot(prot);
    let va = VirtAddr::new(addr);
    let file = if !flags.contains(MmapFlags::MAP_ANONYMOUS) {
        Some(task.with_mut_fdtable(|table| table.get_file(fd as usize))?)
    } else {
        None
    };

    log::info!("[sys_mmap] addr: {addr:#x}, length: {length:#x}, perm: {perm:?}, flags: {flags:?}");

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
    if addr % PAGE_SIZE != 0 {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let addr_space = task.addr_space();
    let prot = MmapProt::from_bits(prot).ok_or(SysError::EINVAL)?;

    log::info!("[sys_mprotect] addr: {addr:#x}, len: {len:#x}, prot: {prot:?}");

    addr_space.change_prot(VirtAddr::new(addr), len, MemPerm::from_mmapprot(prot));
    Ok(0)
}

/// The madvise() system call is used to give advice or directions to the kernel about the
/// address range beginning at address addr and with size length. madvise() only operates
/// on whole pages, therefore addr must be page-aligned. The value of length is rounded up
/// to a multiple of page size. In most cases, the goal of such advice is to improve system
/// or application performance.
///
/// Initially, the system call supported a set of "conventional" advice values, which are
/// also available on several other implementations. (Note, though, that madvise() is not
/// specified in POSIX.) Subsequently, a number of Linux-specific advice values have been added.
pub fn sys_madvise(add: usize, length: usize, _advice: usize) -> SyscallResult {
    log::trace!("[sys_madvise] not implemented add: {add:#x}, length: {length:#x}");
    Ok(0)
}
