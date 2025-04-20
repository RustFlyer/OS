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
    let perm = MemPerm::from_mmapprot(prot);
    let va = VirtAddr::new(addr);

    log::info!("[sys_mmap] addr:{addr:?} prot:{prot:?}, flags:{flags:?}, perm:{perm:?}");

    if addr == 0 && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    task.addr_space()
        .map_file(file, flags, prot, va, length, offset)
}

pub async fn sys_munmap(addr: usize, length: usize) -> SyscallResult {
    let task = current_task();
    let addr = VirtAddr::new(addr);
    task.addr_space().remove_mapping(addr, length);
    Ok(0)
}

pub async fn sys_brk(addr: usize) -> SyscallResult {
    current_task().addr_space().change_heap_size(addr, 0)
}

pub fn sys_mprotect(addr: usize, len: usize, prot: i32) -> SyscallResult {
    if addr == 0 || addr % PAGE_SIZE != 0 {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let addr_space = task.addr_space();
    let prot = MmapProt::from_bits(prot).ok_or(SysError::EINVAL)?;
    addr_space.change_prot(VirtAddr::new(addr), len, MemPerm::from_mmapprot(prot));
    Ok(0)
}
