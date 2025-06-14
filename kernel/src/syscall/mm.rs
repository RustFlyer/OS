use config::mm::PAGE_SIZE;
use id_allocator::IdAllocator;
use mm::address::VirtAddr;
use mutex::new_share_mutex;
use shm::{
    SharedMemory,
    flags::{ShmAtFlags, ShmGetFlags},
    id::ShmStat,
    manager::{SHARED_MEMORY_KEY_ALLOCATOR, SHARED_MEMORY_MANAGER},
};
use systype::{
    error::{SysError, SyscallResult},
    memory_flags::{MappingFlags, MmapFlags, MmapProt},
};

use crate::{processor::current_task, vm::user_ptr::UserWritePtr};

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
    let va = VirtAddr::new(addr);
    let file = if !flags.contains(MmapFlags::MAP_ANONYMOUS) {
        Some(task.with_mut_fdtable(|table| table.get_file(fd as usize))?)
    } else {
        None
    };

    log::info!("[sys_mmap] addr: {addr:#x}, length: {length:#x}, flags: {flags:?}");

    if addr == 0 && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    let result = task.addr_space()
        .map_file(file, flags, MappingFlags::from(prot), va, length, offset);
    log::info!("[sys_mmap] allocated at: {:#x}", result?);
    result
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
    log::debug!("[sys_munmap] addr: {addr:#x}, length: {length:#x}");

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

    addr_space.change_prot(VirtAddr::new(addr), len, MappingFlags::from(prot));
    Ok(0)
}

/// The `madvise()` system call is used to give advice or directions to the kernel about the
/// address range beginning at address addr and with size length. madvise() only operates
/// on whole pages, therefore addr must be page-aligned. The value of length is rounded up
/// to a multiple of page size. In most cases, the goal of such advice is to improve system
/// or application performance.
///
/// Initially, the system call supported a set of "conventional" advice values, which are
/// also available on several other implementations. (Note, though, that madvise() is not
/// specified in POSIX.) Subsequently, a number of Linux-specific advice values have been added.
pub fn sys_madvise(add: usize, length: usize, _advice: usize) -> SyscallResult {
    log::error!("[sys_madvise] not implemented add: {add:#x}, length: {length:#x}");
    Ok(0)
}

/// `shmget()` returns the identifier of the System V shared memory segment associated with
/// the value of the argument key. It may be used either to obtain the identifier of a previously
/// created shared memory segment (when `shmflg` is zero and key does not have the value IPC_PRIVATE),
/// or to create a new set.
///
/// A new shared memory segment, with size equal to the value of size rounded up to a multiple of
/// PAGE_SIZE, is created if key has the value IPC_PRIVATE or key isn't IPC_PRIVATE, no shared memory
/// segment corresponding to key exists, and IPC_CREAT is specified in shmflg.
///
/// If shmflg specifies both IPC_CREAT and IPC_EXCL and a shared memory segment already exists for key,
/// then `shmget()` fails with errno set to EEXIST.
pub fn sys_shmget(key: usize, size: usize, shmflg: i32) -> SyscallResult {
    let shmflg = ShmGetFlags::from_bits_truncate(shmflg);
    let task = current_task();
    log::info!("[sys_shmget] {key} {size} {:?}", shmflg);

    const PAGE_MASK: usize = PAGE_SIZE - 1;
    const IPC_PRIVATE: usize = 0;

    let rounded_up_sz = (size + PAGE_MASK) & !PAGE_MASK;

    if key == IPC_PRIVATE {
        let new_key = SHARED_MEMORY_KEY_ALLOCATOR.lock().alloc().unwrap();
        let new_shm = SharedMemory::new(rounded_up_sz, task.pid());
        SHARED_MEMORY_MANAGER
            .0
            .lock()
            .insert(new_key, new_share_mutex(new_shm));
        return Ok(new_key);
    }

    let mut shm_manager = SHARED_MEMORY_MANAGER.0.lock();

    if let Some(shm) = shm_manager.get(&key) {
        if shmflg.contains(ShmGetFlags::IPC_CREAT | ShmGetFlags::IPC_EXCL) {
            return Err(SysError::EEXIST);
        }
        if shm.lock().size() < size {
            return Err(SysError::EINVAL);
        }
        return Ok(key);
    }

    if !shmflg.contains(ShmGetFlags::IPC_CREAT) {
        return Err(SysError::ENOENT);
    }

    let new_shm = SharedMemory::new(rounded_up_sz, task.pid());
    shm_manager.insert(key, new_share_mutex(new_shm));

    Ok(key)
}

/// `shmat()` attaches the System V shared memory segment identified by `shmid` to the address space of the
/// calling process. The attaching address is specified by `shmaddr` with one of the following criteria:
///
/// - If `shmaddr` is NULL, the system chooses a suitable (unused) page-aligned address to attach the segment.
/// - If `shmaddr` isn't NULL and SHM_RND is specified in `shmflg`, the attach occurs at the address equal to
///   `shmaddr` rounded down to the nearest multiple of SHMLBA.
/// - Otherwise, `shmaddr` must be a page-aligned address at which the attach occurs.
pub fn sys_shmat(shmid: usize, shmaddr: usize, shmflg: i32) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let shmflg = ShmAtFlags::from_bits_truncate(shmflg);
    let shmaddr = VirtAddr::new(shmaddr);

    log::info!("[sys_shmat] {shmid} {shmaddr:?} {:?}", shmflg);

    if shmaddr.page_offset() != 0 && !shmflg.contains(ShmAtFlags::SHM_RND) {
        return Err(SysError::EINVAL);
    }

    let shmaddr_aligned = shmaddr.round_down();
    let mut mem_perm = MmapProt::PROT_READ | MmapProt::PROT_WRITE;
    if shmflg.contains(ShmAtFlags::SHM_EXEC) {
        mem_perm.insert(MmapProt::PROT_EXEC);
    }
    if shmflg.contains(ShmAtFlags::SHM_RDONLY) {
        mem_perm.remove(MmapProt::PROT_WRITE);
    }

    let ret_addr;
    if let Some(shm) = SHARED_MEMORY_MANAGER.0.lock().get(&shmid) {
        ret_addr = addrspace.attach_shm(
            shmaddr_aligned,
            shm.lock().size(),
            shm.clone(),
            MappingFlags::from(mem_perm),
        )?;

        task.with_mut_shm_maps(|map| map.insert(ret_addr, shmid));
    } else {
        return Err(SysError::EINVAL);
    }
    SHARED_MEMORY_MANAGER.attach(shmid, task.pid());
    Ok(ret_addr.into())
}

/// `shmdt()` detaches the shared memory segment located at the address specified by `shmaddr`
/// from the address space of the calling process. The to-be-detached segment must be currently
/// attached with shmaddr equal to the value returned by the attaching shmat() call.
///
/// On a successful `shmdt()` call, the system updates the members of the shmid_ds structure
/// associated with the shared memory segment as follows:
/// - `shm_dtime` is set to the current time.
/// - `shm_lpid` is set to the process-ID of the calling process.
/// - `shm_nattch` is decremented by one. If it becomes 0 and the segment is marked for deletion, the segment is deleted.
pub fn sys_shmdt(shmaddr: usize) -> SyscallResult {
    log::info!("[sys_shmdt] {:#x}", shmaddr);
    let task = current_task();
    let addrspace = task.addr_space();
    let shmaddr = VirtAddr::new(shmaddr);

    if shmaddr.page_offset() != 0 {
        return Err(SysError::EINVAL);
    }

    let mut shmmaps = task.shm_maps_mut().lock();
    let shm_id = shmmaps.remove(&shmaddr);

    if let Some(shm_id) = shm_id {
        addrspace.detach_shm(shmaddr)?;
        SHARED_MEMORY_MANAGER.detach(shm_id, task.pid());
        Ok(0)
    } else {
        Err(SysError::EINVAL)
    }
}

/// `shmctl()` performs the control operation specified by op on the System V shared memory segment whose
/// identifier is given in shmid.
///
/// The `buf` argument is a pointer to a shmid_ds structure, defined in <sys/shm.h> as follows:
/// ```c
/// struct shmid_ds {
///     struct ipc_perm shm_perm;    /* Ownership and permissions */
///     size_t          shm_segsz;   /* Size of segment (bytes) */
///     time_t          shm_atime;   /* Last attach time */
///     time_t          shm_dtime;   /* Last detach time */
///     time_t          shm_ctime;   /* Creation time/time of last
///                                     modification via `shmctl()` */
///     pid_t           shm_cpid;    /* PID of creator */
///     pid_t           shm_lpid;    /* PID of last shmat(2)/shmdt(2) */
///     shmatt_t        shm_nattch;  /* No. of current attaches */
///     ...
/// };
/// ```
#[allow(non_snake_case)]
pub fn sys_shmctl(shmid: usize, cmd: i32, buf: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    match cmd {
        2 => {
            let shm_manager = SHARED_MEMORY_MANAGER.0.lock();
            if let Some(shm) = shm_manager.get(&shmid) {
                let mut buf = UserWritePtr::<ShmStat>::new(buf, &addrspace);
                unsafe { buf.write(shm.lock().stat) }?;
                Ok(0)
            } else {
                Err(SysError::EINVAL)
            }
        }
        0 => Ok(0),
        cmd => {
            log::error!("[sys_shmctl] unimplemented cmd {cmd}");
            Err(SysError::EINVAL)
        }
    }
}

/// The membarrier() system call helps reducing the overhead of the memory barrier instructions
/// required to order memory accesses on multi-core systems. However, this system call is heavier
/// than a memory barrier, so using it effectively is not as simple as replacing memory barriers
/// with this system call, but requires understanding of the details below.
///
/// Use of memory barriers needs to be done taking into account that a memory barrier always
/// needs to be either matched with its memory barrier counterparts, or that the architecture's
/// memory model doesn't require the matching barriers.
///
/// Tips: Unimplement now.
pub fn sys_membarrier(_cmd: usize, _flags: usize, _cpu_id: usize) -> SyscallResult {
    log::error!("[sys_membarrier] not implemented");
    Ok(0)
}

pub fn sys_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: i32,
    new_addr: usize, // when fixed
) -> SyscallResult {
    log::error!("[sys_mremap] not implemented");
    return Ok(0);
    if old_addr & (PAGE_SIZE - 1) != 0 {
        return Err(SysError::EINVAL);
    }
    if new_size == 0 {
        return Err(SysError::EINVAL);
    }

    let task = current_task();
    let src_va = VirtAddr::new(old_addr);
    let copy_size = usize::min(old_size, new_size);

    let new_addr = task.addr_space().map_file(
        None,
        MmapFlags::MAP_ANONYMOUS | MmapFlags::MAP_PRIVATE,
        MappingFlags::R | MappingFlags::W,
        VirtAddr::new(0),
        new_size,
        0,
    )?;

    if copy_size > 0 {
        unsafe {
            let src_ptr = old_addr as *const u8;
            let dst_ptr = new_addr as *mut u8;
            core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, copy_size);
        }
    }

    task.addr_space().remove_mapping(src_va, old_size);
    Ok(new_addr)
}

pub fn sys_mlock(addr: usize, len: usize) -> SyscallResult {
    log::warn!("[sys_mlock] swap page mechanism not implemented");
    Ok(0)
}

pub fn sys_munlock(addr: usize, len: usize) -> SyscallResult {
    log::warn!("[sys_munlock] swap page mechanism not implemented");
    Ok(0)
}
