use crate::{syscall::signal::sys_kill, task::{manager::TASK_MANAGER, process_manager::PROCESS_GROUP_MANAGER, signal::sig_info::Sig, tid::{PGid, Pid}, Task, TaskState}};
use crate::task::signal::sig_info::SigSet;
use crate::task::future::{suspend_now, yield_now};
use crate::vm::user_ptr::{UserReadPtr, UserWritePtr};
use crate::{processor::current_task, task::future::spawn_user_task};
use alloc::boxed::Box;
use alloc::string::ToString;
use bitflags::*;
use config::inode::{InodeMode, InodeType};
use config::process::CloneFlags;
use driver::println;
use log::debug;
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};
use vfs::file::File;
use vfs::path::Path;

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
    let tid = match task.parent_mut().lock().unwrap().upgrade() {
        None => None,
        Some(parent) => Some(parent.tid()),
        };
    if tid == None {
        log::info!("root task trying to exit");
    } else {
        sys_kill(Sig::SIGCHLD.raw() as i32, tid.unwrap() as isize);
    }
    Ok(0)
}

pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}

pub async fn sys_wait() {
    
}

pub async fn sys_waitpid() -> SyscallResult {
    Ok(0)
}

pub async fn sys_wait4(
    pid: i32,
    mut status: UserWritePtr<'_, i32>,
    options: i32
) -> SyscallResult {
    let task = current_task();
    let option = WaitOptions::from_bits_truncate(options);

    let target = match pid {
        -1 => WaitFor::AnyChild,
        0 => WaitFor::AnyChildInGroup,
        p if p > 0 => WaitFor::Pid(p as Pid),
        p => WaitFor::PGid(p as PGid),
    };
    log::info!("[sys_wait4] target: {target:?}, option: {option:?}");

    let res_task = {
        let children = task.children_mut().lock();
        if children.is_empty() {
            log::info!("[sys_wait4] fail: no child");
            return Err(SysError::ECHILD);
        }
        // TODO: check if PG has  
        match target {
            WaitFor::AnyChild => children
                .values()
                // Question: How to handle &&Weak<Task>
                .find(|c| c.upgrade().map_or(false, |t| t.is_in_state(TaskState::Zombie) && t.with_thread_group(|tg| tg.len() == 1))),
            WaitFor::Pid(pid) => {
                if let Some(child) = children.get(&pid) {
                    if child.upgrade().map_or(false, |t| t.is_in_state(TaskState::Zombie) && t.with_thread_group(|tg| tg.len() == 1)) {
                        Some(child)
                    } else {
                        None
                    }
                } else {
                    log::info!("[sys_wait4] fail: no child with pid {pid}");
                    return Err(SysError::ECHILD);
                }
            }
            WaitFor::PGid(_) => unimplemented!(),
            WaitFor::AnyChildInGroup => unimplemented!(),
        }
        .cloned()
    };

    if let Some(res_task) = res_task {
        let zombie_task = res_task.upgrade().unwrap();
        task.timer_mut()
            .update_child_time((zombie_task.timer_mut().user_time(), zombie_task.timer_mut().sys_time()));
        if !status.is_null() {
            // status stores signal in the lowest 8 bits and exit code in higher 8 bits
            let exit_code = zombie_task.get_exit_code();
            log::debug!("[sys_wait4] wstatus: {exit_code:#x}");
            unsafe {
                status.write(exit_code)?;
            }
        }
        let tid = zombie_task.tid();
        task.remove_child(zombie_task.clone());
        TASK_MANAGER.remove_task(tid);
        PROCESS_GROUP_MANAGER.remove(&zombie_task);
        return Ok(tid);
    } else if option.contains(WaitOptions::WNOHANG) {
        return Ok(0);
    } else {
        log::info!("[sys_wait4] waiting for sigchld");
        // 如果等待的进程还不是zombie，那么本进程进行await，
        // 直到等待的进程do_exit然后发送SIGCHLD信号唤醒自己
        let (child_pid, exit_code, child_utime, child_stime) = loop {
            task.set_state(TaskState::Interruptable);
            task.set_wake_up_signal(!task.get_sig_mask() | SigSet::SIGCHLD);
            suspend_now().await;
            task.set_state(TaskState::Running);
            let si = task.sig_manager_mut().get_expect(SigSet::SIGCHLD);
            if let Some(_info) = si {
                let children = task.children_mut().lock();
                let child = match target {
                    WaitFor::AnyChild => children
                        .values()
                        .find(|c| c.upgrade().map_or(false, |t| t.is_in_state(TaskState::Zombie) && t.with_thread_group(|tg| tg.len() == 1))),
                    WaitFor::Pid(pid) => {
                        let child = children.get(&pid).unwrap();
                        if child.upgrade().map_or(false, |t| t.is_in_state(TaskState::Zombie) && t.with_thread_group(|tg| tg.len() == 1)) {
                            Some(child)
                        } else {
                            None
                        }
                    }
                    WaitFor::PGid(_) => unimplemented!(),
                    WaitFor::AnyChildInGroup => unimplemented!(),
                };
                if let Some(child) = child {
                    let child = child.upgrade().unwrap();
                    break (
                        child.tid(),
                        child.get_exit_code(),
                        child.timer_mut().user_time(),
                        child.timer_mut().sys_time(),
                    );
                }
            } else {
                return Err(SysError::EINTR);
            }
        };
        task.timer_mut()
            .update_child_time((child_utime, child_stime));
        if !status.is_null() {
            // status stores signal in the lowest 8 bits and exit code in higher 8 bits
            // status macros can be found in <bits/waitstatus.h>
            log::trace!("[sys_wait4] wstatus: {:#x}", exit_code);
            unsafe {
                status.write(exit_code)?;
            }
        }
        let child = TASK_MANAGER.get_task(child_pid).unwrap();
        task.remove_child(child);
        TASK_MANAGER.remove_task(child_pid);
        PROCESS_GROUP_MANAGER.remove(&task);
        return Ok(child_pid);
    }
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
        let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut addr_space_lock);
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
        path.walk()?
    };

    let file = <dyn File>::open(dentry)?;
    let name = format!("{path:?}");
    Task::spawn_from_elf(file, &name);
    Ok(0)
}


bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    /// Defined in <bits/waitflags.h>.
    pub struct WaitOptions: i32 {
        /// Don't block waiting.
        const WNOHANG = 0x00000001;
        /// Report status of stopped children.
        const WUNTRACED = 0x00000002;
        /// Report continued child.
        const WCONTINUED = 0x00000008;
    }
}

#[derive(Debug)]
        enum WaitFor {
            // wait for any child process in the specific process group
            PGid(PGid),
            // wait for any child process
            AnyChild,
            // wait for any child process in the same process group of the calling process
            AnyChildInGroup,
            // wait for the child process with the specific pid
            Pid(Pid),
        }