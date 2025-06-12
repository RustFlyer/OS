use alloc::vec::Vec;
use bitflags::bitflags;
use systype::error::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    task::{manager::TASK_MANAGER, mask::CpuMask},
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub fn sys_sched_getscheduler() -> SyscallResult {
    log::warn!("[sys_sched_getscheduler] unimplemented");
    Ok(0)
}

pub fn sys_sched_getparam() -> SyscallResult {
    log::warn!("[sys_sched_getparam] unimplemented");
    Ok(0)
}

pub fn sys_sched_setscheduler() -> SyscallResult {
    log::warn!("[sys_sched_setscheduler] unimplemented");
    Ok(0)
}

pub fn sys_sched_setaffinity(pid: usize, cpusetsize: usize, mask: usize) -> SyscallResult {
    log::warn!("[sys_sched_setaffinity] pid: {pid}, cpusetsize: {cpusetsize}");
    let task = current_task();
    let addrspace = task.addr_space();
    let mut mask = UserReadPtr::<CpuMask>::new(mask, &addrspace);
    if cpusetsize < 1 {
        return Err(SysError::EINVAL);
    }
    let real_pid = if pid == 0 { task.pid() } else { pid };
    if let Some(task) = TASK_MANAGER.get_task(real_pid) {
        let mask = unsafe { mask.read() }?;
        *task.cpus_on_mut() = mask;
    } else {
        return Err(SysError::ESRCH);
    }
    Ok(0)
}

pub fn sys_sched_getaffinity(pid: usize, cpusetsize: usize, mask: usize) -> SyscallResult {
    log::warn!("[sys_sched_getaffinity] pid: {pid}, cpusetsize: {cpusetsize}");
    let task = current_task();
    let addrspace = task.addr_space();
    if cpusetsize < 1 {
        return Err(SysError::EINVAL);
    }
    let real_pid = if pid == 0 { task.pid() } else { pid };
    let cpumask_val: usize = if let Some(task) = TASK_MANAGER.get_task(real_pid) {
        (*task.cpus_on_mut()).bits()
    } else {
        return Err(SysError::ESRCH);
    };

    let mask_bytes = cpumask_val.to_le_bytes();

    log::warn!("[sys_sched_getaffinity] mask_bytes: {:x?}", mask_bytes);

    let ulong_size = core::mem::size_of::<usize>();
    let nwords = cpusetsize / ulong_size;
    for i in 0..nwords {
        let mut ptr = UserWritePtr::<usize>::new(mask + i * ulong_size, &addrspace);
        let value = if i == 0 { cpumask_val } else { 0 };
        unsafe {
            ptr.write(value)?;
        }
    }

    log::warn!("[sys_sched_getaffinity] pass");
    Ok(0)
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mempolicy {
    Default = 0,
    Preferred = 1,
    Bind = 2,
    Interleave = 3,
}

impl TryFrom<i32> for Mempolicy {
    type Error = ();
    fn try_from(val: i32) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Mempolicy::Default),
            1 => Ok(Mempolicy::Preferred),
            2 => Ok(Mempolicy::Bind),
            3 => Ok(Mempolicy::Interleave),
            _ => Err(()),
        }
    }
}

bitflags! {
    pub struct MPOLFlags: usize {
        const MPOL_F_NODE = 0x1;
        const MPOL_F_ADDR = 0x2;
        const MPOL_F_MEMS_ALLOWED = 0x4;
    }
}

pub fn sys_get_mempolicy(
    policy_ptr: usize,
    nodelist_ptr: usize,
    maxnode: usize,
    addr: usize,
    flags: isize,
) -> SyscallResult {
    if maxnode == 0 {
        return Err(SysError::EINVAL);
    }
    log::warn!("[sys_get_mempolicy] only support one cpu");
    log::warn!("[sys_get_mempolicy] policy_ptr: {policy_ptr:#x}, nodelist_ptr: {nodelist_ptr:#x}");
    // now only support one cpu
    let task = current_task();
    let addrspace = task.addr_space();
    let cur_policy = Mempolicy::Default as i32;

    if policy_ptr != 0 {
        let mut policy = UserWritePtr::<i32>::new(policy_ptr, &addrspace);
        unsafe { policy.write(cur_policy)? };
    }

    if nodelist_ptr != 0 {
        let ulong_bits = core::mem::size_of::<usize>() * 8;
        let n_nodemask = (maxnode + ulong_bits - 1) / ulong_bits;
        for i in 0..n_nodemask {
            let mut ptr = UserWritePtr::<usize>::new(
                nodelist_ptr + i * core::mem::size_of::<usize>(),
                &addrspace,
            );
            // let value = if i == 0 { 1 } else { 0 };
            let value = 1;
            unsafe {
                ptr.write(value)?;
            }
        }
    }

    Ok(0)
}
