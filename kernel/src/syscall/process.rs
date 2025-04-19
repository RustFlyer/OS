use crate::task::TaskState;
use crate::task::future::{suspend_now, yield_now};
use crate::task::signal::sig_info::SigSet;
use crate::task::{
    manager::TASK_MANAGER,
    process_manager::PROCESS_GROUP_MANAGER,
    tid::{PGid, Pid},
};
use crate::vm::user_ptr::{UserReadPtr, UserWritePtr};
use crate::{processor::current_task, task::future::spawn_user_task};
use alloc::string::String;
use alloc::vec::Vec;
use bitflags::*;
use config::process::CloneFlags;
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};
use vfs::file::File;
use vfs::path::Path;

/// `gettid()` returns the caller's thread ID (TID).  
///
/// # Type
/// - In a single-threaded process, the thread ID is equal to the process ID (PID, as returned by getpid(2)).
/// - In a multi-threaded process, all threads have the same PID, but each one has a unique TID.
pub fn sys_gettid() -> SyscallResult {
    Ok(current_task().tid())
}

/// getpid() returns the process ID (PID) of the calling process.
pub fn sys_getpid() -> SyscallResult {
    Ok(current_task().pid())
}

/// `getppid()` returns the process ID of the parent of the calling process. This will be either the
/// ID of the process that created this process using `fork()`, or, if that process has already terminated,
/// the ID of the process to which this process has been reparented.
///
/// # Tips
/// - If the caller's parent is in a different PID namespace, `getppid()` returns 0.
/// - From a kernel perspective, the PID is sometimes also known as the thread group ID (TGID).
///   This contrasts with the kernel thread ID (TID), which is unique for each thread.
pub fn sys_getppid() -> SyscallResult {
    let r = current_task().ppid();
    log::info!("[sys_getppid] ppid: {r:?}");
    Ok(r)
}

/// `exit()` system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are performed
/// only if this is the last thread in the thread group.
pub fn sys_exit(_exit_code: i32) -> SyscallResult {
    let task = current_task();
    task.set_state(TaskState::Zombie);
    if task.is_process() {
        task.set_exit_code((_exit_code & 0xFF) << 8);
    }
    Ok(0)
}

/// `sched_yield()`  causes the calling thread to relinquish the CPU.  The thread is moved to the end
/// of the queue for its static priority and a new thread gets to run.
///
/// # Tips
/// - If the calling thread is the only thread in the highest priority list at that time, it will continue
///   to run after a call to `sched_yield()`.
pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}

pub async fn sys_wait4(pid: i32, wstatus: usize, options: i32) -> SyscallResult {
    // log::error!("[sys_wait4] in");
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
                .find(|c| {
                    c.is_in_state(TaskState::Zombie) && c.with_thread_group(|tg| tg.len() == 1)
                }),
            WaitFor::Pid(pid) => {
                if let Some(child) = children.get(&pid) {
                    if child.is_in_state(TaskState::Zombie)
                        && child.with_thread_group(|tg| tg.len() == 1)
                    {
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
        let addr_space = task.addr_space();
        let mut status = UserWritePtr::<i32>::new(wstatus, &addr_space);
        let zombie_task = res_task;
        task.timer_mut().update_child_time((
            zombie_task.timer_mut().user_time(),
            zombie_task.timer_mut().sys_time(),
        ));
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
        // log::error!("[sys_wait4] remove tid {}", tid);
        TASK_MANAGER.remove_task(tid);
        PROCESS_GROUP_MANAGER.remove(&zombie_task);
        Ok(tid)
    } else if option.contains(WaitOptions::WNOHANG) {
        Ok(0)
    } else {
        log::info!("[sys_wait4] waiting for sigchld");
        // 如果等待的进程还不是zombie，那么本进程进行await，
        // 直到等待的进程do_exit然后发送SIGCHLD信号唤醒自己
        let (child_pid, exit_code, child_utime, child_stime) = loop {
            task.set_state(TaskState::Interruptable);
            task.set_wake_up_signal(!task.get_sig_mask() | SigSet::SIGCHLD);
            // log::info!("suspend_now again");
            suspend_now().await;
            // log::info!("return from suspend");
            task.set_state(TaskState::Running);
            let si = task.sig_manager_mut().get_expect(SigSet::SIGCHLD);
            if let Some(_info) = si {
                // log::info!("siginfo get");
                let children = task.children_mut().lock();

                let child = match target {
                    WaitFor::AnyChild => children.values().find(|c| {
                        c.is_in_state(TaskState::Zombie) && c.with_thread_group(|tg| tg.len() == 1)
                    }),

                    WaitFor::Pid(pid) => {
                        let child = children.get(&pid).unwrap();
                        if child.is_in_state(TaskState::Zombie)
                            && child.with_thread_group(|tg| tg.len() == 1)
                        {
                            Some(child)
                        } else {
                            None
                        }
                    }

                    WaitFor::PGid(_) => unimplemented!(),

                    WaitFor::AnyChildInGroup => unimplemented!(),
                };
                if let Some(child) = child {
                    break (
                        child.tid(),
                        child.get_exit_code(),
                        child.timer_mut().user_time(),
                        child.timer_mut().sys_time(),
                    );
                }
                // log::info!("siginfo end");
            } else {
                log::info!("return SysError::EINTR");
                return Err(SysError::EINTR);
            }
        };
        // log::info!("timer_mut get and update_child_time");
        task.timer_mut()
            .update_child_time((child_utime, child_stime));

        // log::info!("addrspace get and write status");
        let addr_space = task.addr_space();
        let mut status = UserWritePtr::<i32>::new(wstatus, &addr_space);
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
        log::error!("[sys_wait4] remove child_pid {}", child_pid);
        TASK_MANAGER.remove_task(child_pid);
        PROCESS_GROUP_MANAGER.remove(&task);
        // log::error!("[sys_wait4] out");
        Ok(child_pid)
    }
}

/// `clone()` create a new ("child") process.
/// The system call provides more precise control over what pieces of execution
/// context are shared between the calling process and the child process.
///
/// # CloneFlag
/// - `CLONE_CHILD_CLEARTID`: Clear  (zero)  the  child thread ID at the location pointed to by child_tid
///   (clone()) in child memory when the child exits, and do a wakeup on the futex at that address.
///   The address involved may be changed by the `set_tid_address` system call.This is used by threading
///   libraries.
/// - `CLONE_CHILD_SETTID`: Store the child thread ID at the location pointed to by child_tid(clone())
///   in the child's memory. The store operation completes before the clone call returns control to
///   user space in the child process.
/// - `CLONE_SETTLS`: The TLS (Thread Local Storage) descriptor is set to tls.
///   The interpretation of tls and the resulting effect is architecture dependent.
///   On architectures with a dedicated TLS register, it is the new value of that register.
/// - `CLONE_PARENT_SETTID`: Store the child thread ID at the location pointed to by parent_tid (clone())
///   in the parent's memory. The store operation completes before the clone call returns
///   control to user space.
pub async fn sys_clone(
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

    let new_task = current_task().fork(flags).await;
    new_task.trap_context_mut().set_user_a0(0);
    let new_tid = new_task.tid();
    log::info!("[sys_clone] clone a new thread, tid {new_tid}, clone flags {flags:?}",);

    current_task().add_child(new_task.clone());

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

/// `execve()` executes the program referred to by `path`. This causes the program that is
/// being run by the calling process to be replaced with a new program, with new stack, heap
/// and (initialized and uninitialized) data segments.
/// # Args
/// - `path` must be either a binary executable, or a script starting with a line of the form:
///   #!interpreter \[optional-arg\]
/// - `argv` is an array of argument strings passed to the new program.
/// - `envp` is an array of strings, conventionally of the form key=value, which are passed as
///   environment to the new program.
///
/// # Tips
/// - The argv and envp arrays must each include a null pointer at the end of the array.
/// - If the current program is being ptraced, a SIGTRAP signal is sent to it after a successful `execve()`.
///
/// # Type
/// - If the executable is an a.out dynamically linked binary executable containing shared-library
///   stubs, the Linux dynamic linker ld.so(8) is called at the start of execution to bring needed
///   shared objects into memory and link the executable with them.
/// - If the executable is a dynamically linked ELF executable, the interpreter named in the PT_INTERP
///   segment is used to load the needed shared objects. This interpreter is typically /lib/ld-linux.so.2
///   for binaries linked with glibc
///
/// # Interpreter scripts
/// An interpreter script is a text file that has execute permission enabled and whose first line
/// is of the form:
/// > #!interpreter \[optional-arg\]
///
/// The interpreter must be a valid pathname for an executable file.
/// For portable use, optional-arg should either be absent, or be specified as a single word
pub async fn sys_execve(path: usize, argv: usize, envp: usize) -> SyscallResult {
    let task = current_task();

    let read_string = |addr| {
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<u8>::new(addr, &addr_space);
        user_ptr
            .read_c_string(256)?
            .into_string()
            .map_err(|_| SysError::EINVAL)
    };

    // Reads strings from a null-terminated array of pointers to strings, adding them to
    // the specified vector.
    let read_string_array = |addr: usize, vec: &mut Vec<String>| {
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<usize>::new(addr, &addr_space);
        let pointers = user_ptr.read_ptr_array(256)?;
        for ptr in pointers {
            let mut user_ptr = UserReadPtr::<u8>::new(ptr, &addr_space);
            let string = user_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;
            vec.push(string);
        }
        Ok(())
    };

    let path = read_string(path)?;
    let mut args = vec![path.clone()];
    read_string_array(argv, &mut args)?;
    let mut envs = Vec::new();
    read_string_array(envp, &mut envs)?;

    log::info!("[sys_execve] args: {args:?}");
    log::info!("[sys_execve] envs: {envs:?}");
    log::info!("[sys_execve] path: {path:?}");

    let dentry = {
        let path = Path::new(sys_root_dentry(), path.clone());
        path.walk()?
    };

    // log::info!("[sys_execve]: open file");
    let file = <dyn File>::open(dentry)?;
    task.execve(file, args, envs, path)?;
    // log::info!("[sys_execve]: finish execve and convert to a new task");
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
        const WCONTINUED = 0x00000004;
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

/// `sys_set_tid_address()` set pointer to thread ID.
///  For each thread, the kernel maintains two attributes (addresses) called `set_child_tid` and
///  `clear_child_tid`. These two attributes contain the value **NULL** by default.
///
/// # Type
/// - **set_child_tid**: If a thread is started using `clone`(2) with the `CLONE_CHILD_SETTID` flag,
///   `set_child_tid` is set to the value passed in the `ctid` argument of that system call.
///   When `set_child_tid` is set, the very first thing the new thread does is to write its
///   thread ID at this `address`.
/// - **clear_child_tid**: If a thread is started using clone(2) with the `CLONE_CHILD_CLEARTID` flag,
///   `clear_child_tid` is set to the value passed in the `ctid` argument of that system call.
///
/// # Tips
/// When a thread whose `clear_child_tid` is **not NULL** terminates, then, if the thread is sharing memory
/// with other threads, then 0 is written at the address specified in clear_child_tid and the kernel
/// performs the following operation:
/// > futex(clear_child_tid, FUTEX_WAKE, 1, NULL, NULL, 0);
///  
/// The effect of this operation is to wake a single thread that is performing a `futex` wait on  the
/// memory location. Errors from the futex wake operation are ignored.
pub fn sys_set_tid_address(tidptr: usize) -> SyscallResult {
    let task = current_task();
    // log::info!("[sys_set_tid_address] tidptr:{tidptr:#x}");
    task.tid_address_mut().clear_child_tid = Some(tidptr);
    Ok(task.tid())
}
