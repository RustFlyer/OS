use alloc::vec::Vec;

use log::info;

use config::process::CloneFlags;
use driver::println;
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};
use vfs::file::File;
use vfs::path::Path;

use crate::task::TaskState;
use crate::task::future::yield_now;
use crate::vm::user_ptr::UserReadPtr;
use crate::{processor::current_task, task::future::spawn_user_task};

pub fn sys_gettid() -> SyscallResult {
    Ok(current_task().tid())
}

/// getpid() returns the process ID (PID) of the calling process.
pub fn sys_getpid() -> SyscallResult {
    Ok(current_task().pid())
}

pub fn sys_getppid() -> SyscallResult {
    let r = current_task().ppid();
    info!("[sys_getppid] ppid: {r:?}");
    Ok(r)
}

/// _exit() system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are
/// performed only if this is the last thread in the thread group.
pub fn sys_exit(exit_code: i32) -> SyscallResult {
    let task = current_task();
    task.set_state(TaskState::Zombie);
    // non-leader thread are detached (see CLONE_THREAD flag in manual page clone.2)
    log::info!("task [{}] exit with {}", task.get_name(), exit_code);
    if task.is_process() {
        task.set_exit_code((exit_code & 0xFF) << 8);
    }
    Ok(0)
}

pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}

pub async fn sys_waitpid() -> SyscallResult {
    Ok(0)
}

pub fn sys_clone(
    flags: usize,
    stack: usize,
    parent_tid_ptr: usize,
    tls_ptr: usize,
    chilren_tid_ptr: usize,
) -> SyscallResult {
    log::info!(
        "[sys_clone] flags:{flags:?}, stack:{stack:#x}, tls:{tls_ptr:?}, parent_tid:{parent_tid_ptr:?}, child_tid:{chilren_tid_ptr:?}"
    );
    let _exit_signal = flags & 0xff;
    let flags = CloneFlags::from_bits(flags as u64 & !0xff).ok_or(SysError::EINVAL)?;
    log::info!("[sys_clone] flags {flags:?}");

    let new_task = current_task().fork(flags);
    new_task.trap_context_mut().set_user_a0(0);
    let new_tid = new_task.tid();
    log::info!("[sys_clone] clone a new thread, tid {new_tid}, clone flags {flags:?}",);

    if stack != 0 {
        new_task.trap_context_mut().set_user_sp(stack);
    }

    if flags.contains(CloneFlags::PARENT_SETTID) {}
    if flags.contains(CloneFlags::CHILD_SETTID) {}
    if flags.contains(CloneFlags::CHILD_CLEARTID) {}
    if flags.contains(CloneFlags::SETTLS) {
        new_task.trap_context_mut().set_user_tp(tls_ptr);
    }

    spawn_user_task(new_task);

    Ok(new_tid)
}

pub fn sys_execve(path: usize, argv: usize, envp: usize) -> SyscallResult {
    let task = current_task();

    let read_string = |addr| {
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut user_ptr = UserReadPtr::<u8>::new(addr, &mut addr_space_lock);
        user_ptr
            .read_c_string(256)?
            .into_string()
            .map_err(|_| SysError::EINVAL)
    };

    let read_string_array = |addr| {
        let mut strings = Vec::new();
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut user_ptr = UserReadPtr::<usize>::new(addr, &mut addr_space_lock);
        let pointers = user_ptr.read_ptr_array(256)?;
        for ptr in pointers {
            let mut user_ptr = UserReadPtr::<u8>::new(ptr, &mut addr_space_lock);
            let string = user_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;
            strings.push(string);
        }
        Ok(strings)
    };

    let path = read_string(path)?;
    let args = read_string_array(argv)?;
    let envs = read_string_array(envp)?;

    println!("args: {:?}", args);
    println!("envs: {:?}", envs);
    log::info!("[sys_execve]: path: {path:?}",);
    let dentry = {
        let path = Path::new(sys_root_dentry(), sys_root_dentry(), &path);
        path.walk()?
    };

    let file = <dyn File>::open(dentry)?;
    task.execve(file, args, envs, path)?;
    Ok(0)
}

pub fn sys_set_tid_address(tidptr: usize) -> SyscallResult {
    let task = current_task();
    log::info!("[sys_set_tid_address] tidptr:{tidptr:#x}");
    task.tid_address_mut().clear_child_tid = Some(tidptr);
    Ok(task.tid())
}
