use crate::task::TaskState;
use crate::{processor::current_task, task::future::spawn_user_task};
use config::process::CloneFlags;
use systype::{SysError, SyscallResult};

use crate::task::future::yield_now;

pub fn sys_gettid() -> SyscallResult {
    Ok(current_task().tid())
}

/// getpid() returns the process ID (PID) of the calling process.
pub fn sys_getpid() -> SyscallResult {
    Ok(current_task().pid())
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
    _parent_tid_ptr: usize,
    _tls_ptr: usize,
    chilren_tid_ptr: usize,
) -> SyscallResult {
    let _exit_signal = flags & 0xff;
    let flags = CloneFlags::from_bits(flags as u64 & !0xff).ok_or(SysError::EINVAL)?;
    log::info!("[sys_clone] flags {flags:?}");

    let new_task = current_task().fork(flags);
    new_task.trap_context_mut().set_user_a0(0);
    let new_tid = new_task.tid();
    log::info!("[sys_clone] clone a new thread, tid {new_tid}, clone flags {flags:?}",);
    spawn_user_task(new_task);
    Ok(new_tid)
}

pub fn sys_wait4(pid: i32, status: i32) {}
