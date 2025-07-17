use systype::error::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    task::{manager::TASK_MANAGER, process_manager::PROCESS_GROUP_MANAGER},
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
/// Returns the real user ID of the calling process.
pub fn sys_getuid() -> SyscallResult {
    let _cred = current_task().perm_mut();
    let cred = _cred.lock();

    Ok(cred.ruid as usize)
}

/// Returns the real group ID of the calling process.
pub fn sys_getgid() -> SyscallResult {
    let _cred = current_task().perm_mut();
    let cred = _cred.lock();

    Ok(cred.rgid as usize)
}

/// `setgid` sets the effective group ID of the calling process.
/// If the calling process is privileged (root), the real GID and saved set-group-ID are also set.
pub fn sys_setgid(gid: usize) -> SyscallResult {
    let _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    // Only root (euid == 0) can set all three GIDs, others can only set egid to their own rgid/egid/sgid
    if cred.euid == 0 {
        cred.rgid = gid as u32;
        cred.egid = gid as u32;
        cred.sgid = gid as u32;
    } else if gid as u32 == cred.rgid || gid as u32 == cred.egid || gid as u32 == cred.sgid {
        cred.egid = gid as u32;
    } else {
        return Err(SysError::EPERM);
    }

    Ok(0)
}

pub fn sys_setreuid(ruid: usize, euid: usize) -> SyscallResult {
    let _cred = current_task().perm_mut();
    let mut cred_lock = _cred.lock();

    // Only root (euid == 0) can set all three UIDs, others can only set euid to their own ruid/euid/suid
    if cred_lock.euid == 0 {
        cred_lock.ruid = ruid as u32;
        cred_lock.euid = euid as u32;
        cred_lock.suid = euid as u32;
    } else if ruid as u32 == cred_lock.ruid
        || ruid as u32 == cred_lock.euid
        || ruid as u32 == cred_lock.suid
    {
        cred_lock.euid = euid as u32;
    } else {
        return Err(SysError::EPERM);
    }

    Ok(0)
}

/// `setuid` sets the effective user ID of the calling process.
/// If the calling process is privileged (root), the real UID and saved set-user-ID are also set.
pub fn sys_setuid(uid: usize) -> SyscallResult {
    let _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    // Only root (euid == 0) can set all three UIDs, others can only set euid to their own ruid/euid/suid
    if cred.euid == 0 {
        cred.ruid = uid as u32;
        cred.euid = uid as u32;
        cred.suid = uid as u32;
    } else if uid as u32 == cred.ruid || uid as u32 == cred.euid || uid as u32 == cred.suid {
        cred.euid = uid as u32;
    } else {
        return Err(SysError::EPERM);
    }

    Ok(0)
}

/// Returns the process group ID (PGID) of the specified process.
/// If pid is zero, returns the PGID of the calling process.
pub fn sys_getpgid(pid: usize) -> SyscallResult {
    let task = if pid != 0 {
        TASK_MANAGER.get_task(pid).ok_or(SysError::ESRCH)?
    } else {
        current_task()
    };
    let _cred = task.perm_mut();
    let mut cred = _cred.lock();

    Ok(cred.pgid as usize)
}

/// setpgid() sets the PGID of the process specified by pid to pgid.
/// If pid is zero, uses the calling process.
/// If pgid is zero, sets PGID to the PID of the process.
pub fn sys_setpgid(pid: usize, pgid: usize) -> SyscallResult {
    let task = if pid != 0 {
        TASK_MANAGER.get_task(pid).ok_or(SysError::ESRCH)?
    } else {
        current_task()
    };

    let new_pgid = if pgid == 0 { task.pid() } else { pgid };

    // Add to process group, create if not exist
    if PROCESS_GROUP_MANAGER.get_group(new_pgid).is_none() {
        PROCESS_GROUP_MANAGER.add_group(&task);
    } else {
        PROCESS_GROUP_MANAGER.add_process(new_pgid, &task);
    }

    // Update the process's PGID in its credentials
    let _cred = task.perm_mut();
    let mut cred = _cred.lock();
    cred.pgid = new_pgid as u32;

    Ok(0)
}

/// Returns the effective user ID of the calling process.
pub fn sys_geteuid() -> SyscallResult {
    let _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    Ok(cred.euid as usize)
}

/// Returns the effective group ID of the calling process.
pub fn sys_getegid() -> SyscallResult {
    let _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    Ok(cred.egid as usize)
}

pub fn sys_setresuid(ruid: isize, euid: isize, suid: isize) -> SyscallResult {
    let mut _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    log::error!("[sys_setresuid] ruid: {ruid}, euid: {euid}, suid: {suid}");
    let uid = cred.euid;
    if uid != 0 {
        if (ruid != -1
            && ruid as u32 != cred.ruid
            && ruid as u32 != cred.euid
            && ruid as u32 != cred.suid)
            || (euid != -1
                && euid as u32 != cred.ruid
                && euid as u32 != cred.euid
                && euid as u32 != cred.suid)
            || (suid != -1
                && suid as u32 != cred.ruid
                && suid as u32 != cred.euid
                && suid as u32 != cred.suid)
        {
            return Err(SysError::EPERM);
        }
    }

    if ruid != -1 {
        cred.ruid = ruid as u32;
    }
    if euid != -1 {
        cred.euid = euid as u32;
    }
    if suid != -1 {
        cred.suid = suid as u32;
    }
    Ok(0)
}

pub fn sys_setresgid(rgid: isize, egid: isize, sgid: isize) -> SyscallResult {
    let mut _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

    log::error!("[sys_setresgid] rgid: {rgid}, egid: {egid}, sgid: {sgid}");
    let uid = cred.euid;
    if uid != 0 {
        if (rgid != -1
            && rgid as u32 != cred.rgid
            && rgid as u32 != cred.egid
            && rgid as u32 != cred.sgid)
            || (egid != -1
                && egid as u32 != cred.rgid
                && egid as u32 != cred.egid
                && egid as u32 != cred.sgid)
            || (sgid != -1
                && sgid as u32 != cred.rgid
                && sgid as u32 != cred.egid
                && sgid as u32 != cred.sgid)
        {
            return Err(SysError::EPERM);
        }
    }

    if rgid != -1 {
        cred.rgid = rgid as u32;
    }
    if egid != -1 {
        cred.egid = egid as u32;
    }
    if sgid != -1 {
        cred.sgid = sgid as u32;
    }
    Ok(0)
}

pub fn sys_getsid(pid: usize) -> SyscallResult {
    let task = if pid != 0 {
        TASK_MANAGER.get_task(pid).ok_or(SysError::ESRCH)?
    } else {
        current_task()
    };
    let _cred = task.perm_mut();
    let cred = _cred.lock();

    Ok(cred.sid as usize)
}

pub fn sys_setsid() -> SyscallResult {
    let task = current_task();
    let mut _cred = task.perm_mut();
    let mut cred = _cred.lock();

    if task.pid() == cred.pgid as usize {
        return Err(SysError::EPERM);
    }

    cred.sid = task.pid() as u32;
    cred.pgid = task.pid() as u32;
    drop(cred);

    PROCESS_GROUP_MANAGER.add_group(&task);

    Ok(task.pid() as usize)
}

pub fn sys_getgroups(size: usize, list_ptr: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let _cred = task.perm_mut();
    let cred = _cred.lock();

    let groups: &[u32] = &cred.groups;
    let ngroups = groups.len();

    if size == 0 {
        return Ok(ngroups);
    }

    if size < ngroups {
        return Err(SysError::EINVAL);
    }

    let mut list_ptr = UserWritePtr::<u32>::new(list_ptr, &addrspace);
    if !list_ptr.is_null() {
        unsafe { list_ptr.write_array(groups)? };
    }

    Ok(ngroups)
}

pub fn sys_setgroups(size: usize, list_ptr: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let _cred = task.perm_mut();
    let mut cred = _cred.lock();

    if cred.euid != 0 {
        return Err(SysError::EPERM);
    }
    if size > 128 {
        return Err(SysError::EINVAL);
    }

    let mut list_ptr = UserReadPtr::<u32>::new(list_ptr, &addrspace);

    if size > 0 {
        if list_ptr.is_null() {
            return Err(SysError::EFAULT);
        }
        unsafe { cred.groups = list_ptr.read_array(size)? };
    }
    Ok(0)
}

pub fn sys_fadvise64_64(fd: usize, offset: usize, len: usize, advice: i32) -> SyscallResult {
    let task = current_task();
    let _file =
        task.with_mut_fdtable(|fdtable| fdtable.get_file(fd).map_err(|_| SysError::EBADF))?;

    // Currently, we do not implement any specific file advice handling.
    // This is a placeholder for future implementation.
    log::info!(
        "[sys_fadvise64_64] fd: {}, offset: {}, len: {}, advice: {}",
        fd,
        offset,
        len,
        advice
    );

    enum Advice {
        Normal = 0,
        Random = 1,
        Sequential = 2,
        WillNeed = 3,
        DontNeed = 4,
        NoReuse = 5,
    }

    if advice < 0 || advice > Advice::NoReuse as i32 {
        return Err(SysError::EINVAL);
    }

    Ok(0)
}
