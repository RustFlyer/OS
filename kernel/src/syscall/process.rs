use crate::task::{Task, TaskState};
use crate::vm::user_ptr::UserReadPtr;
use crate::{processor::current_task, task::future::spawn_user_task};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use config::inode::{InodeMode, InodeType};
use config::process::CloneFlags;
use log::debug;
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};
use vfs::file::File;
use vfs::path::Path;

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
    _stack: usize,
    _parent_tid_ptr: usize,
    _tls_ptr: usize,
    _chilren_tid_ptr: usize,
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

pub fn sys_execve(path: usize, _argv: usize, _envp: usize) -> SyscallResult {
    let task = current_task();

    let read_c_str = |addr| {
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut *addr_space_lock);
        match data_ptr.read_c_string(30) {
            Ok(data) => match core::str::from_utf8(&data) {
                Ok(utf8_str) => utf8_str.to_string(),
                Err(_) => unimplemented!(),
            },
            Err(_) => unimplemented!(),
        }
    };

    let path = read_c_str(path);

    log::info!("[sys_execve]: path: {path:?}",);
    let dentry = {
        let path = Path::new(sys_root_dentry(), sys_root_dentry(), &path);
        path.walk().expect("sys_openat: fail to find dentry")
    };

    let inode = dentry.inode().unwrap();
    inode.set_inotype(InodeType::from(InodeMode::REG));

    let file = <dyn File>::open(dentry)?;

    let elf_data = Box::new(file.read_all()?);
    let elf_data_u8: &'static [u8] = Box::leak(elf_data);

    debug!("add len:{}", elf_data_u8.len());

    let name = format!("{path:?}");
    Task::spawn_from_elf(elf_data_u8, &name);
    Ok(0)
}
