use systype::error::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    task::{manager::TASK_MANAGER, process_manager::PROCESS_GROUP_MANAGER},
};

/// Returns the real user ID of the calling process.
pub fn sys_getuid() -> SyscallResult {
    let task = current_task();
    Ok(task.uid())
}

/// Returns the real group ID of the calling process.
pub fn sys_getgid() -> SyscallResult {
    let task = current_task();
    Ok(task.get_pgid())
}

/// `setgid` sets the effective group ID of the calling process.
/// If the calling process is privileged, the real GID and saved set-group-ID are also set.
///
/// more precisely: has the CAP_SETGID capability in its user namespace.
///
/// In linux, every processes (tasks) has its own **real group ID (RGID), effective group ID (EGID)
/// and saved set-group-ID (SGID)**.
/// Therefore, any process is under its main group and this group's id is GID.
///
/// For threads, they will share the gid of their process.
///
/// Typical application: After the daemon process starts, it first starts as root,
/// and then sets gid/gid to a regular user to reduce permissions and enhance system security.
pub fn sys_setgid(gid: usize) -> SyscallResult {
    // log::warn!("[sys_setgid] unimplemented call gid: {gid}");
    let task = current_task();
    task.set_pgid(gid);
    Ok(0)
}

/// `setuid` sets user id of current system account.
/// `uid` is a number used by the operating system to uniquely identify a user.
///
/// Each process is running with a "UID" identity.
///
/// Typical application: A daemon process first performs sensitive tasks as root,
/// and then setuid(1000) returns to a regular user to continue working stably
/// and enhance security.
pub fn sys_setuid(uid: usize) -> SyscallResult {
    log::debug!("[sys_setuid]  uid: {uid}");
    let task = current_task();
    *task.uid_lock().lock() = uid;
    Ok(0)
}

/// `getpgid` gets pgid from thread with specified id `pid`.
/// If `pid` is zero, the function will return current task pgid.
pub fn sys_getpgid(pid: usize) -> SyscallResult {
    let task = if pid != 0 {
        TASK_MANAGER.get_task(pid).ok_or(SysError::ENOMEM)?
    } else {
        current_task()
    };
    let pgid = task.get_pgid();

    Ok(pgid)
}

/// setpgid() sets the PGID of the process specified by pid to pgid.
///
/// # Exception
/// - If `pid` is zero, then the process ID of the calling process is used.
/// - If `pgid` is zero, then the PGID of the process specified by pid is made the
///   same as its process ID.
///
/// If setpgid() is used to move a process from one process group to another (as is
/// done by some shells when creating pipelines), both process groups must be part of
/// the same session (see setsid(2) and credentials(7)).
///
/// In this case, the pgid specifies an existing process group to be joined
/// and the session ID of that group must match the session ID of the joining process.
pub fn sys_setpgid(pid: usize, pgid: usize) -> SyscallResult {
    let task = if pid != 0 {
        TASK_MANAGER.get_task(pid).ok_or(SysError::ESRCH)?
    } else {
        current_task()
    };

    if pgid == 0 {
        PROCESS_GROUP_MANAGER.add_group(&task);
    } else if PROCESS_GROUP_MANAGER.get_group(pgid).is_none() {
        PROCESS_GROUP_MANAGER.add_group(&task);
    } else {
        PROCESS_GROUP_MANAGER.add_process(pgid, &task);
    }

    Ok(0)
}

/// `geteuid()` returns the effective user ID of the calling process.
pub fn sys_geteuid() -> SyscallResult {
    let euid = current_task().uid();
    log::debug!("[sys_geteuid] euid: {} now return 9", euid);
    Ok(euid)
}

/// `getegid()` returns the effective group ID of the calling process.
pub fn sys_getegid() -> SyscallResult {
    let egid = current_task().get_pgid();
    log::debug!("[sys_getegid] egid: {}", egid);
    Ok(egid)
}

pub fn sys_setresuid(ruid: isize, euid: isize, suid: isize) -> SyscallResult {
    let mut _cred = current_task().perm_mut();
    let mut cred = _cred.lock();

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
