use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::task;
use alloc::vec::Vec;

use bitflags::*;
use config::vfs::OpenFlags;
use osfs::simple::dentry::SimpleDentry;
use osfs::special::perf::event::PerfEventAttr;
use osfs::special::perf::file::PerfEventFile;
use osfs::special::perf::flags::{PERF_ATTR_SIZE_VER0, PerfType};
use osfs::special::perf::inode::PerfEventInode;
use strum::FromRepr;

use config::inode::InodeType;
use config::mm::USER_STACK_SIZE;
use config::process::CloneFlags;
use osfs::sys_root_dentry;
use osfuture::{suspend_now, yield_now};
use systype::rusage::Rusage;
use systype::{
    error::{SysError, SyscallResult},
    rlimit::RLimit,
};
use vfs::file::File;
use vfs::path::Path;

use crate::logging::enable_log;
use crate::task::cap::{CapUserData, CapUserHeader, CapabilitiesFlags};
use crate::task::signal::pidfd::PF_TABLE;
use crate::task::signal::sig_info::{Sig, SigInfo};
use crate::task::{
    TaskState,
    manager::TASK_MANAGER,
    process_manager::PROCESS_GROUP_MANAGER,
    signal::sig_info::SigSet,
    tid::{PGid, Pid},
};
use crate::vm::user_ptr::{UserReadPtr, UserWritePtr};
use crate::{processor::current_task, task::future::spawn_user_task};
use signal::LinuxSigInfo;

use crate::task::wait_queue::WAIT_QUEUE_MANAGER;

/// `gettid` returns the caller's thread ID (TID).
///
/// # Type
/// - In a single-threaded process, the thread ID is equal to the process ID (PID, as returned by getpid(2)).
/// - In a multi-threaded process, all threads have the same PID, but each one has a unique TID.
pub fn sys_gettid() -> SyscallResult {
    log::info!("[sys_gettid] call");
    Ok(current_task().tid())
}

/// `getpid` returns the process ID (PID) of the calling process.
pub fn sys_getpid() -> SyscallResult {
    Ok(current_task().pid())
}

/// `getppid` returns the process ID of the parent of the calling process. This will be either the
/// ID of the process that created this process using `fork`, or, if that process has already terminated,
/// the ID of the process to which this process has been reparented.
///
/// # Tips
/// - If the caller's parent is in a different PID namespace, `getppid` returns 0.
/// - From a kernel perspective, the PID is sometimes also known as the thread group ID (TGID).
///   This contrasts with the kernel thread ID (TID), which is unique for each thread.
pub fn sys_getppid() -> SyscallResult {
    let r = current_task().ppid();
    // log::info!("[sys_getppid] ppid: {r:?}");
    Ok(r)
}

/// `exit()` system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are performed
/// only if this is the last thread in the thread group.
pub fn sys_exit(status: i32) -> SyscallResult {
    if status == 114514 {
        panic!("114514!");
    }
    let task = current_task();
    task.set_state(TaskState::Zombie);
    if task.is_process() {
        task.set_exit_code((status & 0xFF) << 8);
    }
    Ok(0)
}

/// `exit_group` system call terminates all threads in the calling thread group.
///
/// # Note
/// The current implementation now supports multi-threading.
pub fn sys_exit_group(status: i32) -> SyscallResult {
    let thread_group = current_task().thread_group_mut();
    let thread_group_lock = thread_group.lock();

    thread_group_lock.iter().for_each(|thread| {
        thread.set_state(TaskState::Zombie);
        thread.wake();
        if thread.is_process() {
            thread.set_exit_code((status & 0xFF) << 8);
        }
    });

    Ok(0)
}

/// `sched_yield`  causes the calling thread to relinquish the CPU.  The thread is moved to the end
/// of the queue for its static priority and a new thread gets to run.
///
/// # Tips
/// - If the calling thread is the only thread in the highest priority list at that time, it will continue
///   to run after a call to `sched_yield`.
pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}

/// "wait4" system call waits for a child process to exit and send SIGCHLD to the waiter.
/// after receiving SIGCHLD, the waiter should recycle the children on WaitForRecycle state.
/// (only process can be set to WaitForRecycle state, threads will be dropped when hart leaves this task)
/// the target "pid" can be:
/// - -1(AnyChild): wait for any child process of current process
/// - 0(AnyChildInGroup): wait for any child process in the same process group of the calling process
/// - >0(Pid): wait for the child process of current process with the specific pid
/// - <0(PGid): wait for any child process in the process group with the specific pgid
pub async fn sys_wait4(pid: i32, wstatus: usize, options: i32) -> SyscallResult {
    // Check for INT_MIN which cannot be negated safely
    if pid == i32::MIN {
        return Err(SysError::ESRCH);
    }

    let task = current_task();
    log::info!(
        "[sys_wait4] task {} called wait4(pid: {pid:?}, wstatus: {wstatus:?}, options: {options:?})",
        task.tid()
    );
    let option = WaitOptions::from_bits(options).ok_or(SysError::EINVAL)?;
    let target = match pid {
        -1 => WaitFor::AnyChild,
        0 => WaitFor::AnyChildInGroup,
        p if p > 0 => WaitFor::Pid(p as Pid),
        p => WaitFor::PGid((-p) as PGid),
    };
    log::info!("[sys_wait4] target: {target:?}, option: {option:?}, wstatus: {wstatus:#x}");
    // log::info!(
    //     "[sys_wait4] existing task number: {}",
    //     TASK_MANAGER.how_many_tasks()
    // );

    // get the child for recycle according to the target
    // NOTE: recycle no more than one child per `sys_wait4`
    let child_for_recycle = match target {
        WaitFor::AnyChild => {
            let children = task.children_mut().lock();
            if children.is_empty() {
                log::warn!(
                    "[sys_wait4] task {} [{}] wait4 fail at beginning: no child",
                    task.tid(),
                    task.get_name()
                );
                return Err(SysError::ECHILD);
            }
            children
                .values()
                .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                .cloned()
        }
        WaitFor::Pid(pid) => {
            let children = task.children_mut().lock();
            if children.is_empty() {
                log::warn!(
                    "[sys_wait4] task {} [{}] wait4 fail at beginning: no child",
                    task.tid(),
                    task.get_name()
                );
                return Err(SysError::ECHILD);
            }
            if let Some(child) = children.get(&pid) {
                if child.is_in_state(TaskState::WaitForRecycle) {
                    Some(child.clone())
                } else {
                    None
                }
            } else {
                log::warn!(
                    "[sys_wait4] task {} [{}] wait4 fail at beginning: no child with pid {pid}",
                    task.tid(),
                    task.get_name()
                );
                return Err(SysError::ECHILD);
            }
        }
        WaitFor::PGid(pgid) => {
            let mut result = None;
            for process in PROCESS_GROUP_MANAGER
                .get_group(pgid)
                .ok_or(SysError::ECHILD)?
                .into_iter()
                .filter_map(|t| t.upgrade())
                .filter(|t| t.is_process())
            {
                let children = process.children_mut().lock();
                if let Some(child) = children
                    .values()
                    .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                {
                    result = Some(child.clone());
                    break;
                }
            }
            result
        }
        WaitFor::AnyChildInGroup => {
            let pgid = task.get_pgid();
            let mut result = None;
            for process in PROCESS_GROUP_MANAGER
                .get_group(pgid)
                .ok_or(SysError::ECHILD)?
                .into_iter()
                .filter_map(|t| t.upgrade())
                .filter(|t| t.is_process())
            {
                let children = process.children_mut().lock();
                if let Some(child) = children
                    .values()
                    .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                {
                    result = Some(child.clone());
                    break;
                }
            }
            result
        }
    };

    if let Some(child_for_recycle) = child_for_recycle {
        log::info!(
            "[sys_wait4] task {} found a child task {} for recycle at the beginning",
            task.tid(),
            child_for_recycle.tid()
        );
        // 1. if there is a child for recycle when `sys_wait4` is called
        let addr_space = task.addr_space();
        let mut status = UserWritePtr::<i32>::new(wstatus, &addr_space);
        let zombie_task = child_for_recycle;
        task.timer_mut().update_child_time((
            zombie_task.timer_mut().user_time(),
            zombie_task.timer_mut().kernel_time(),
        ));
        if !status.is_null() {
            // status stores signal in the lowest 8 bits and exit code in higher 8 bits
            let exit_code = zombie_task.get_exit_code();
            unsafe {
                status.write(exit_code)?;
            }
        }
        let tid = zombie_task.tid();

        task.remove_child(zombie_task.clone());

        TASK_MANAGER.remove_task(tid);

        PROCESS_GROUP_MANAGER.remove(&zombie_task);
        Ok(tid)
    } else if option.contains(WaitOptions::WNOHANG) {
        // 2. if WNOHANG option is set and there is no child for recycle, return immediately
        log::info!(
            "[sys_wait4] task {} wait4 return 0 because of WNOHANG",
            task.tid()
        );
        Ok(0)
    } else {
        // 3. if there is no child for recycle and WNOHANG option is not set, wait in wait queue
        WAIT_QUEUE_MANAGER.add_waiter(task.clone(), target.clone());

        log::info!(
            "[sys_wait4] task [{}] suspend using wait queue for target: {:?}",
            task.get_name(),
            target
        );
        task.set_state(TaskState::Interruptible);
        suspend_now().await;
        task.set_state(TaskState::Running);

        let (child_tid, exit_code, child_utime, child_stime) = {
            // check if there is a child for recycle
            // NOTE: no loop here, only continue waiting if user set SA_RESTART or loop call `sys_wait4`
            let child = match target {
                WaitFor::AnyChild => {
                    let children = task.children_mut().lock();
                    children
                        .values()
                        .find(|c| {
                            c.is_in_state(TaskState::WaitForRecycle)
                                && c.with_thread_group(|tg| tg.len() == 1)
                        })
                        .cloned()
                }
                WaitFor::Pid(pid) => {
                    let children = task.children_mut().lock();
                    if let Some(child) = children.get(&pid) {
                        if child.is_in_state(TaskState::WaitForRecycle)
                            && child.with_thread_group(|tg| tg.len() == 1)
                        {
                            Some(child.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                WaitFor::PGid(pgid) => {
                    let mut result = None;
                    for process in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                        .filter(|t| t.is_process())
                    {
                        log::info!(
                            "[sys_wait4] in PGid block, task {} try to find the assigned child task after suspending(target: {:?})",
                            task.tid(),
                            target
                        );
                        let children = process.children_mut().lock();
                        if let Some(child) = children
                            .values()
                            .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                        {
                            result = Some(child.clone());
                            break;
                        }
                    }
                    result
                }
                WaitFor::AnyChildInGroup => {
                    log::info!(
                        "[sys_wait4] in AnyChildInGroup block, task {} try to find the assigned child task after suspending(target: {:?})",
                        task.tid(),
                        target
                    );
                    let pgid = task.get_pgid();
                    let mut result = None;

                    let btree = PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?;
                    log::info!("[sys_wait4] pgid {} group length: {}", pgid, btree.len());
                    for task in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                    {
                        log::debug!("[sys_wait4] task {} in pgid {} group", task.tid(), pgid);
                    }

                    for process in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                        .filter(|t| t.is_process())
                    {
                        let children = process.children_mut().lock();
                        if let Some(child) = children
                            .values()
                            .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                        {
                            result = Some(child.clone());
                            break;
                        }
                    }
                    result
                }
            };

            if let Some(child) = child {
                log::info!(
                    "[sys_wait4] task {} found the child task {} for recycle after suspending",
                    task.tid(),
                    child.tid()
                );
                // 从等待队列中移除当前任务
                WAIT_QUEUE_MANAGER.remove_waiter(&task);
                (
                    child.tid(),
                    child.get_exit_code(),
                    child.timer_mut().user_time(),
                    child.timer_mut().kernel_time(),
                )
            } else {
                // 检查是否被信号中断
                if task
                    .sig_manager_mut()
                    .has_expect_signals(!*task.sig_mask_mut())
                {
                    log::info!("[sys_wait4] task {} interrupted by signal", task.tid());
                    WAIT_QUEUE_MANAGER.remove_waiter(&task);
                }
                log::debug!(
                    "[sys_wait4] task {} will continue waiting if SA_RESTART is set",
                    task.tid()
                );
                return Err(SysError::EINTR);
            }
        };

        // log::info!("timer_mut get and update_child_time");
        task.timer_mut()
            .update_child_time((child_utime, child_stime));

        let addr_space = task.addr_space();
        let mut status = UserWritePtr::<i32>::new(wstatus, &addr_space);
        // if wstatus is not null, write the exit code of child to wstatus
        if !status.is_null() {
            // status stores signal in the lowest 8 bits and exit code in higher 8 bits
            // status macros can be found in <bits/waitstatus.h>
            // log::trace!("[sys_wait4] wstatus: {:#x}", exit_code);
            unsafe {
                status.write(exit_code)?;
            }
        }
        // check if the child is still in TASK_MANAGER
        if let Some(child) = TASK_MANAGER.get_task(child_tid) {
            log::info!(
                "[sys_wait4] parent task {} remove task [{}] with tid [{}] after suspending",
                task.tid(),
                child.get_name(),
                child_tid
            );
            // remove the child from current task's children, and TASK_MANAGER, thus the child will be dropped after hart leaves child
            // NOTE: the child's thread group itself will be recycled when the child is dropped, and it use Weak pointer so it won't affect the drop of child
            PROCESS_GROUP_MANAGER.remove(&child);
            task.remove_child(child);
        } else {
            // Child already removed from TASK_MANAGER, just log and continue
            log::warn!(
                "[sys_wait4] parent task {} can't find child task {} in TASK_MANAGER after suspending",
                task.tid(),
                child_tid
            );
            // Still need to remove from parent's children list by tid
            if let Some(child) = task.children_mut().lock().remove(&child_tid) {
                PROCESS_GROUP_MANAGER.remove(&child);
                log::debug!(
                    "[sys_wait4] removed not found child task {} from parent's children list",
                    child_tid
                );
            }
        }

        TASK_MANAGER.remove_task(child_tid);
        Ok(child_tid)
    }
}

#[derive(Debug, Clone)]
pub enum WaitFor {
    // wait for any child process in the specific process group
    PGid(PGid),
    // wait for any child process
    AnyChild,
    // wait for any child process in the same process group of the calling process
    AnyChildInGroup,
    // wait for the child process with the specific pid
    Pid(Pid),
}

/// `clone` create a new ("child") process.
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
///
/// # Note for architecture differences
/// The order of the arguments differs between architectures.
/// - On RISC-V, the order is: flags, stack, parent_tid, tls, child_tid.
/// - On LoongArch, the order is: flags, stack, parent_tid, child_tid, tls.
pub fn sys_clone(arg1: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> SyscallResult {
    let (flags, stack, parent_tid, child_tid, tls) = {
        #[cfg(target_arch = "riscv64")]
        {
            (arg1, arg2, arg3, arg5, arg4)
        }
        #[cfg(target_arch = "loongarch64")]
        {
            (arg1, arg2, arg3, arg4, arg5)
        }
    };
    __sys_clone(flags, stack, parent_tid, child_tid, tls)
}

/// This is the actual implementation of `sys_clone`. It is separated from `sys_clone` to
/// allow for different argument orders on different architectures.
fn __sys_clone(
    flags: usize,
    stack: usize,
    parent_tid_ptr: usize,
    child_tid_ptr: usize,
    tls_ptr: usize,
) -> SyscallResult {
    log::info!(
        "[sys_clone] flags:{flags:#x}, stack:{stack:#x}, tls:{tls_ptr:#x}, parent_tid:{parent_tid_ptr:#x}, child_tid:{child_tid_ptr:x}"
    );
    let task = current_task();
    let addrspace = task.addr_space();
    let _exit_signal = flags & 0xff;
    let flags = CloneFlags::from_bits(flags as u64 & !0xff).ok_or(SysError::EINVAL)?;
    log::info!("[sys_clone] flags {flags:?}");

    let new_task = task.fork(flags);
    new_task.trap_context_mut().set_user_ret_val(0);
    let new_tid = new_task.tid();
    log::info!("[sys_clone] clone a new thread, tid {new_tid}, clone flags {flags:?}",);

    if stack != 0 {
        new_task.trap_context_mut().set_user_sp(stack);
    }

    if flags.contains(CloneFlags::PARENT_SETTID) {
        let mut parent_tid = UserWritePtr::<usize>::new(parent_tid_ptr, &addrspace);
        if !parent_tid.is_null() {
            unsafe { parent_tid.write(new_tid)? };
        }
    }

    if flags.contains(CloneFlags::CHILD_SETTID) {
        let mut child_tid = UserWritePtr::<usize>::new(child_tid_ptr, &addrspace);
        log::info!("[sys_clone] clone a new thread, tid {new_tid}",);
        unsafe { child_tid.write(new_tid)? };
        new_task.tid_address_mut().set_child_tid = Some(child_tid_ptr);
    }

    if flags.contains(CloneFlags::CHILD_CLEARTID) {
        new_task.tid_address_mut().clear_child_tid = Some(child_tid_ptr);
    }

    if flags.contains(CloneFlags::SETTLS) {
        new_task.trap_context_mut().set_user_tp(tls_ptr);
    }

    log::info!("[sys_clone] who is your parent? {}", new_task.ppid());
    spawn_user_task(new_task);
    log::info!("[sys_clone] clone success",);

    if flags.contains(CloneFlags::VFORK) {
        task.set_state(TaskState::Sleeping);
    }

    Ok(new_tid)
}

/// `execve` executes the program referred to by `path`. This causes the program that is
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
/// - If the current program is being ptraced, a SIGTRAP signal is sent to it after a successful `execve`.
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

    const PATH_LEMGTH: usize = 4096;

    let read_string = |addr| {
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<u8>::new(addr, &addr_space);
        user_ptr
            .read_c_string(PATH_LEMGTH)?
            .into_string()
            .map_err(|_| SysError::EINVAL)
    };

    // Reads strings from a null-terminated array of pointers to strings, adding them to
    // the specified vector.
    let read_string_array = |addr: usize| {
        let mut args = Vec::new();
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<usize>::new(addr, &addr_space);
        let pointers = user_ptr.read_ptr_array(PATH_LEMGTH)?;
        for ptr in pointers {
            let mut user_ptr = UserReadPtr::<u8>::new(ptr, &addr_space);
            let string = user_ptr
                .read_c_string(PATH_LEMGTH)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;
            args.push(string);
        }
        Ok(args)
    };

    let mut path = read_string(path)?;
    let mut args = read_string_array(argv)?;
    let mut envs = read_string_array(envp)?;

    if path.is_empty() {
        log::warn!("[sys_execve] path is empty");
        return Err(SysError::ENOENT);
    }

    // DEBUG
    path = path.replace("mkfs.ext3", "mkfs.ext2");
    path = path.replace("mkfs.ext4", "mkfs.ext2");
    path = path.replace("mkfs.exfat", "mkfs.ext2");
    path = path.replace("mkfs.bcachefs", "mkfs.ext2");
    path = path.replace("mkfs.btrfs", "mkfs.ext2");
    path = path.replace("mkfs.xfs", "mkfs.ext2");

    log::info!("[sys_execve] task: {:?}", task.get_name());
    log::info!("[sys_execve] args: {args:?}");
    log::info!("[sys_execve] envs: {envs:?}");
    log::info!("[sys_execve] path: {path:?}");

    let dentry = {
        let root = if path.starts_with("/") {
            sys_root_dentry()
        } else {
            task.cwd().lock().clone()
        };

        let path = Path::new(root, path);
        let dentry = path.walk()?;
        if !dentry.is_negative() && dentry.inode().unwrap().inotype() == InodeType::SymLink {
            Path::resolve_symlink_through(Arc::clone(&dentry))?
        } else {
            dentry
        }
    };
    if dentry.is_negative() {
        log::warn!("[sys_execve] file not found");
        return Err(SysError::ENOENT);
    }
    if dentry.inode().unwrap().inotype() != InodeType::File {
        log::warn!("[sys_execve] not a regular file");
        return Err(SysError::EACCES);
    }

    let filepath = dentry.path();
    let expath = filepath.rsplit_once('/').map(|x| x.0).unwrap_or("");
    let argpath = format!(
        "PATH={}:/bin:/sbin:/usr/bin:/usr/local/bin:/usr/local/sbin:ltp/testcases/bin:",
        expath
    );
    envs.push(argpath);

    let arghome = "HOME=/";
    envs.push(arghome.to_string());

    log::info!("[sys_execve]: open file {}", dentry.path());
    let file = <dyn File>::open(dentry)?;

    let mut name = String::new();

    // DEBUG
    args.iter_mut().for_each(|arg| {
        *arg = arg.replace("mkfs.ext3", "mkfs.ext2");
        *arg = arg.replace("mkfs.ext4", "mkfs.ext2");
        *arg = arg.replace("mkfs.exfat", "mkfs.ext2");
        *arg = arg.replace("mkfs.bcachefs", "mkfs.ext2");
        *arg = arg.replace("mkfs.btrfs", "mkfs.ext2");
        *arg = arg.replace("mkfs.xfs", "mkfs.ext2");
    });

    args.iter().for_each(|arg| {
        name.push_str(arg);
        name.push(' ');
    });

    let result = task.execve(file.clone(), args.clone(), envs.clone(), name);
    if result == Err(SysError::ENOEXEC) {
        let mut first_line = vec![0; 128];
        file.seek(config::vfs::SeekFrom::Start(0))?;
        file.read(&mut first_line).await?;
        if let Some(index) = first_line.iter().position(|&b| b == b'\n') {
            first_line.truncate(index);
        }

        let interpreter_cmd = String::from_utf8(first_line).map_or(String::from("/bin/sh"), |s| {
            s.strip_prefix("#!")
                .map_or(String::from("/bin/sh"), |s| s.trim().to_string())
        });

        let mut interpreter_args: Vec<String> = interpreter_cmd
            .split_whitespace()
            .map(String::from)
            .collect();
        interpreter_args.extend(args);
        if interpreter_args[0].ends_with("bash") {
            // We don't have bash, so we substitute it with sh as a workaround
            interpreter_args[0] = String::from("/bin/sh");
        }

        let interpreter_path = interpreter_args[0].clone();

        log::info!("[sys_execve]: execute as a script: {:?}", interpreter_args);

        let interpreter_dentry = {
            let cwd = task.cwd().lock().clone();
            let path = Path::new(cwd, interpreter_path.clone());
            let dentry = path.walk()?;
            if !dentry.is_negative() && dentry.inode().unwrap().inotype() == InodeType::SymLink {
                Path::resolve_symlink_through(dentry)?
            } else {
                dentry
            }
        };
        if interpreter_dentry.is_negative() {
            log::warn!("[sys_execve]: interpreter not found: {}", interpreter_path);
            return Err(SysError::ENOENT);
        }

        let interpreter_file = <dyn File>::open(interpreter_dentry)?;
        let cmdline = interpreter_args.join(" ");
        task.execve(interpreter_file, interpreter_args, envs, cmdline)?;
    } else if let Err(e) = result {
        return Err(e);
    }

    if let Some(parent) = task.vfork_parent.clone() {
        if let Some(task) = parent.upgrade() {
            task.wake();
        }
    }

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

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    /// Defined in <sys/wait.h> and related headers.
    pub struct WaitIdOptions: i32 {
        /// Don't block waiting.
        const WNOHANG = 0x00000001;
        /// Report status of stopped children.
        const WSTOPPED = 0x00000002;
        /// Report status of terminated children.
        const WEXITED = 0x00000004;
        /// Report continued child.
        const WCONTINUED = 0x00000008;
        /// Leave the child in a waitable state.
        const WNOWAIT = 0x01000000;
    }
}

/// `sys_set_tid_address` set pointer to thread ID.
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
    log::info!("[sys_set_tid_address] tidptr:{tidptr:#x}");
    task.tid_address_mut().clear_child_tid = Some(tidptr);
    Ok(task.tid())
}

#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Resource {
    // Per-process CPU limit, in seconds.
    CPU = 0,
    // Largest file that can be created, in bytes.
    FSIZE = 1,
    // Maximum size of data segment, in bytes.
    DATA = 2,
    // Maximum size of stack segment, in bytes.
    STACK = 3,
    // Largest core file that can be created, in bytes.
    CORE = 4,
    // Largest resident set size, in bytes.
    // This affects swapping; processes that are exceeding their
    // resident set size will be more likely to have physical memory
    // taken from them.
    RSS = 5,
    // Number of processes.
    NPROC = 6,
    // Number of open files.
    NOFILE = 7,
    // Locked-in-memory address space.
    MEMLOCK = 8,
    // Address space limit.
    AS = 9,
    // Maximum number of file locks.
    LOCKS = 10,
    // Maximum number of pending signals.
    SIGPENDING = 11,
    // Maximum bytes in POSIX message queues.
    MSGQUEUE = 12,
    // Maximum nice priority allowed to raise to.
    // Nice levels 19 .. -20 correspond to 0 .. 39
    // values of this resource limit.
    NICE = 13,
    // Maximum realtime priority allowed for non-priviledged
    // processes.
    RTPRIO = 14,
    // Maximum CPU time in microseconds that a process scheduled under a real-time
    // scheduling policy may consume without making a blocking system
    // call before being forcibly descheduled.
    RTTIME = 15,
}

/// `prlimit()` system call combines and extends the functionality of `setrlimit()` and `getrlimit()`.
/// It can be used to both set and get the resource limits of an arbitrary process.
///
/// If the `new_limit` argument is not NULL, then the rlimit structure to which it points is
/// used to set new values for the soft and hard limits for resource.
///
/// If the `old_limit` argument is not NULL, then a successful call to `prlimit()` places the
/// previous soft and hard limits for resource in the rlimit structure pointed to by `old_limit`.
///
///
/// The pid argument specifies the ID of the process on which the call is to operate.
/// If pid is 0, then the call applies to the calling process.
///```c
/// struct rlimit {
///     rlim_t rlim_cur;  /* Soft limit */
///     rlim_t rlim_max;  /* Hard limit (ceiling for rlim_cur) */
/// };
/// ```
pub fn sys_prlimit64(
    pid: usize,
    resource: i32,
    new_limit: usize,
    old_limit: usize,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let mut nlimit = UserReadPtr::<RLimit>::new(new_limit, &addrspace);
    let mut olimit = UserWritePtr::<RLimit>::new(old_limit, &addrspace);

    let _ptask = if pid == 0 {
        task.clone()
    } else {
        TASK_MANAGER.get_task(pid).ok_or(SysError::EINVAL)?
    };

    let resource = Resource::from_repr(resource).ok_or(SysError::EINVAL)?;

    log::debug!("[prlimit64] pid: {pid}, resource: {resource:?}");

    if !olimit.is_null() {
        let limit = match resource {
            Resource::STACK => RLimit::one(USER_STACK_SIZE, USER_STACK_SIZE),
            Resource::NOFILE => task.with_mut_fdtable(|table| table.get_rlimit()),
            r => {
                log::error!("[sys_prlimit64] old limit {:?} not implemented", r);
                RLimit::one(0, 0)
            }
        };
        unsafe { olimit.write(limit)? };
    }

    if !nlimit.is_null() {
        let rlimit = unsafe { nlimit.read()? };
        match resource {
            Resource::STACK => {
                log::debug!("[sys_prlimit64] new limit STACK: {:?}", rlimit);
            }
            Resource::NOFILE => {
                task.with_mut_fdtable(|table| table.set_rlimit(rlimit));
            }
            r => {
                log::error!("[sys_prlimit64] new limit {:?} not implemented", r);
            }
        }
    }

    Ok(0)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CloneArgs {
    pub flags: u64,
    pub pidfd: u64,
    pub child_tid: u64,
    pub parent_tid: u64,
    pub exit_signal: u64,
    pub stack: u64,
    pub stack_size: u64,
    pub tls: u64,
    pub set_tid: u64,
    pub set_tid_size: u64,
    pub cgroup: u64,
}

pub fn sys_clone3(user_args: usize, size: usize) -> SyscallResult {
    // log::error!("[sys_clone3] called");
    if size < core::mem::size_of::<CloneArgs>() {
        return Err(SysError::EINVAL);
    }

    if size > core::mem::size_of::<CloneArgs>() {
        return Err(SysError::EFAULT);
    }

    let task = current_task();
    let addrspace = task.addr_space();
    let mut args_ptr = UserReadPtr::<CloneArgs>::new(user_args, &addrspace);
    let args = unsafe { args_ptr.read()? };

    let exit_signal = args.exit_signal as i32;
    if exit_signal != 0 && !Sig::from_i32(exit_signal).is_valid() {
        return Err(SysError::EINVAL);
    }

    log::info!(
        "[sys_clone3] flags:0x{:x}, stack=0x{:x}, tls=0x{:x} parent_tid=0x{:x} child_tid=0x{:x} exit_signal={}",
        args.flags,
        args.stack,
        args.tls,
        args.parent_tid,
        args.child_tid,
        args.exit_signal
    );

    // log::error!("[sys_clone3] {:?}", args);

    let flags = CloneFlags::from_bits(args.flags & !0xff).ok_or(SysError::EINVAL)?;
    let new_task = task.fork(flags);
    new_task.trap_context_mut().set_user_ret_val(0);
    let new_tid = new_task.tid();

    if flags.contains(CloneFlags::SIGHAND) && !flags.contains(CloneFlags::VM) {
        return Err(SysError::EINVAL);
    }

    if flags.contains(CloneFlags::THREAD)
        && (!flags.contains(CloneFlags::SIGHAND) || !flags.contains(CloneFlags::VM))
    {
        return Err(SysError::EINVAL);
    }

    if args.stack != 0 && args.stack_size == 0 {
        return Err(SysError::EINVAL);
    }

    if args.stack != 0 {
        new_task
            .trap_context_mut()
            .set_user_sp(args.stack as usize + USER_STACK_SIZE);
    }

    if flags.contains(CloneFlags::PARENT_SETTID) && args.parent_tid != 0 {
        let mut parent_tid = UserWritePtr::<usize>::new(args.parent_tid as usize, &addrspace);
        unsafe { parent_tid.write(new_tid)? };
    }

    if flags.contains(CloneFlags::CHILD_SETTID) && args.child_tid != 0 {
        let mut child_tid = UserWritePtr::<usize>::new(args.child_tid as usize, &addrspace);
        unsafe { child_tid.write(new_tid)? };
        new_task.tid_address_mut().set_child_tid = Some(args.child_tid as usize);
    }

    if flags.contains(CloneFlags::CHILD_CLEARTID) {
        new_task.tid_address_mut().clear_child_tid = Some(args.child_tid as usize);
    }

    if flags.contains(CloneFlags::SETTLS) && args.tls != 0 {
        new_task.trap_context_mut().set_user_tp(args.tls as usize);
    }

    if flags.contains(CloneFlags::PIDFD) && args.pidfd != 0 {
        let pidfd = PF_TABLE.get().unwrap().new_pidfd(&new_task);
        let mut user_pidfd = UserWritePtr::<usize>::new(args.pidfd as usize, &addrspace);
        unsafe { user_pidfd.write(pidfd)? };
    }

    *new_task.exit_signal.lock() = Some(exit_signal as u8);

    log::info!("[sys_clone3] who is your parent? {}", new_task.ppid());
    spawn_user_task(new_task);
    log::info!("[sys_clone3] clone success");

    Ok(new_tid)
}

#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum RusageType {
    RUSAGE_SELF = 0,
    RUSAGE_CHILDREN = -1,
    RUSAGE_THREAD = 1,
}

pub fn sys_getrusage(who: i32, usage: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut usageptr = UserWritePtr::<Rusage>::new(usage, &addrspace);
    let who = RusageType::from_repr(who).ok_or(SysError::EINVAL)?;
    let mut ret = Rusage::default();
    log::debug!("[sys_getrusage] thread: {}, who: {who:?}", task.tid());
    match who {
        RusageType::RUSAGE_SELF => {
            let (total_utime, total_stime) = task.get_process_ustime();
            ret.utime = total_utime.into();
            ret.stime = total_stime.into();
            log::debug!("[sys_getrusage] ret: {ret:?}");
            unsafe {
                usageptr.write(ret)?;
            }
        }
        RusageType::RUSAGE_CHILDREN => {
            let (total_utime, total_stime) = task.get_children_ustime();
            ret.utime = total_utime.into();
            ret.stime = total_stime.into();
            unsafe {
                usageptr.write(ret)?;
            }
        }
        RusageType::RUSAGE_THREAD => {
            let (total_utime, total_stime) = task.get_thread_ustime();
            ret.utime = total_utime.into();
            ret.stime = total_stime.into();
            unsafe {
                usageptr.write(ret)?;
            }
        }
    }
    Ok(0)
}

pub fn sys_capget(hdrp: usize, datap: usize) -> SyscallResult {
    use crate::task::cap::*;
    let task = current_task();
    let addrspace = task.addr_space();

    let mut hdr_ptr = UserReadPtr::<CapUserHeader>::new(hdrp, &addrspace);
    let hdr = unsafe { hdr_ptr.read()? };

    log::debug!("[sys_capget] hdr: {hdr:?}");

    let mut hdr_write_ptr = UserWritePtr::<CapUserHeader>::new(hdrp, &addrspace);

    let u32s = match hdr.version {
        _LINUX_CAPABILITY_VERSION_1 => CAPABILITY_U32S_1,
        _LINUX_CAPABILITY_VERSION_2 | _LINUX_CAPABILITY_VERSION_3 => CAPABILITY_U32S_2,
        _ => {
            let mut new_hdr = hdr;
            new_hdr.version = _LINUX_CAPABILITY_VERSION_3;
            unsafe {
                hdr_write_ptr.write(new_hdr)?;
            }
            return Err(SysError::EINVAL);
        }
    };

    if hdr.pid < -1 {
        return Err(SysError::EINVAL);
    }
    let target_pid = if hdr.pid == 0 || hdr.pid == -1 {
        task.pid() as i32
    } else {
        hdr.pid
    };
    if target_pid != task.pid() as i32 {
        return Err(SysError::ESRCH);
    }

    unsafe {
        hdr_write_ptr.write(hdr)?;
    }

    let mut data_ptr = UserWritePtr::<CapUserData>::new(datap, &addrspace);
    let caps = &task.capability();
    let slice = unsafe { data_ptr.try_into_mut_slice(u32s) }?;
    for i in 0..u32s {
        let data = CapUserData {
            effective: caps.effective[i],
            permitted: caps.permitted[i],
            inheritable: caps.inheritable[i],
        };
        slice[i] = data;
    }
    Ok(0)
}

pub fn sys_capset(hdrp: usize, datap: usize) -> SyscallResult {
    use crate::task::cap::*;
    let task = current_task();
    let addrspace = task.addr_space();

    let mut hdr_ptr = UserReadPtr::<CapUserHeader>::new(hdrp, &addrspace);
    let hdr = unsafe { hdr_ptr.read()? };

    log::debug!("[sys_capset] hdr: {hdr:?}");
    let u32s = match hdr.version {
        _LINUX_CAPABILITY_VERSION_1 => CAPABILITY_U32S_1,
        _LINUX_CAPABILITY_VERSION_2 | _LINUX_CAPABILITY_VERSION_3 => CAPABILITY_U32S_2,
        _ => return Err(SysError::EINVAL),
    };

    if hdr.pid != 0 && hdr.pid != task.pid() as i32 {
        return Err(SysError::EPERM);
    }

    let mut data_ptr = UserReadPtr::<CapUserData>::new(datap, &addrspace);
    let caps = &mut task.capability();
    let slice = unsafe { data_ptr.try_into_slice(u32s) }?;

    // 1. 检查 effective 必须是 new permitted 的子集
    for i in 0..u32s {
        let data = slice[i];
        if data.effective & !data.permitted != 0 {
            log::error!("[sys_capset] return EINVAL (effective not subset of permitted)");
            return Err(SysError::EINVAL);
        }
    }

    // 2. 检查 new permitted 只能是 old permitted 的子集（只能降权）
    for i in 0..u32s {
        let data = slice[i];
        if data.permitted & !caps.permitted[i] != 0 {
            log::error!("[sys_capset] return EPERM (permitted not subset of old permitted)");
            return Err(SysError::EPERM);
        }
    }

    // 3. 检查 new inheritable 只能是 (old inheritable | new permitted) 的子集
    for i in 0..u32s {
        let data = slice[i];
        if data.inheritable & !(caps.inheritable[i] | data.permitted) != 0 {
            log::error!(
                "[sys_capset] return EPERM (inheritable not subset of old_inheritable | new_permitted)"
            );
            return Err(SysError::EPERM);
        }
    }

    for i in 0..u32s {
        let data = slice[i];
        caps.effective[i] = data.effective;
        caps.permitted[i] = data.permitted;
        caps.inheritable[i] = data.inheritable;
    }

    Ok(0)
}

pub fn sys_prctl(
    option: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> SyscallResult {
    use core::sync::atomic::Ordering::Relaxed;
    pub const PR_SET_NAME: usize = 15;
    pub const PR_GET_NAME: usize = 16;
    pub const PR_SET_DUMPABLE: usize = 4;
    pub const PR_GET_DUMPABLE: usize = 3;
    pub const PR_SET_NO_NEW_PRIVS: usize = 38;
    pub const PR_GET_NO_NEW_PRIVS: usize = 39;
    pub const PR_SET_PDEATHSIG: usize = 1;
    pub const PR_GET_PDEATHSIG: usize = 2;

    let task = current_task();
    let addrspace = task.addr_space();

    log::debug!("[sys_prctl] option: {}", option);

    match option {
        PR_SET_NAME => {
            let mut name_ptr = UserReadPtr::<u8>::new(arg2, &addrspace);
            let name = unsafe { name_ptr.try_into_slice(16)? };
            *task.name_mut() = String::from_utf8(name.to_vec()).unwrap();
            Ok(0)
        }
        PR_GET_NAME => {
            let mut name_ptr = UserWritePtr::<u8>::new(arg2, &addrspace);
            let name = task.get_name();
            unsafe {
                name_ptr.write_array(name.as_bytes())?;
            }
            Ok(0)
        }
        PR_SET_DUMPABLE => {
            // arg2: 0/1
            task.dumpable.store(arg2 != 0, Relaxed);
            Ok(0)
        }
        PR_GET_DUMPABLE => Ok(task.dumpable.load(Relaxed) as usize),
        PR_SET_NO_NEW_PRIVS => {
            // arg2: 0/1
            task.no_new_privs.store(arg2 != 0, Relaxed);
            Ok(0)
        }
        PR_GET_NO_NEW_PRIVS => Ok(task.no_new_privs.load(Relaxed) as usize),
        PR_SET_PDEATHSIG => {
            // arg2: signal id
            task.pdeathsig.store(arg2 as u32, Relaxed);
            Ok(0)
        }
        PR_GET_PDEATHSIG => {
            let mut sig_ptr = UserWritePtr::<u32>::new(arg2, &addrspace);
            unsafe {
                sig_ptr.write(task.pdeathsig.load(Relaxed))?;
            }
            Ok(0)
        }
        _ => Err(SysError::EINVAL),
    }
}

pub fn sys_chroot(path_ptr: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();

    let path = UserReadPtr::<u8>::new(path_ptr, &addr_space).read_c_string(4096)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    const VFS_MAXNAMELEN: usize = 255;
    if path.len() > VFS_MAXNAMELEN {
        return Err(SysError::ENAMETOOLONG);
    }

    let dentry = task.walk_at(config::vfs::AtFd::FdCwd, path)?;

    let inode = dentry.inode().ok_or(SysError::ENOENT)?;
    if !inode.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }

    *task.root().lock() = dentry;
    Ok(0)
}

pub fn sys_perf_event_open(
    attr_ptr: usize,
    pid: i32,
    cpu: i32,
    group_fd: i32,
    flags: u64,
) -> SyscallResult {
    use vfs::inode::Inode;
    let task = current_task();

    log::debug!(
        "[sys_perf_event_open] attr_ptr: {:#x}, pid: {}, cpu: {}, group_fd: {}, flags: {:#x}",
        attr_ptr,
        pid,
        cpu,
        group_fd,
        flags
    );

    if attr_ptr == 0 {
        return Err(SysError::EFAULT);
    }

    const PERF_FLAG_FD_NO_GROUP: u64 = 1 << 0;
    const PERF_FLAG_FD_OUTPUT: u64 = 1 << 1;
    const PERF_FLAG_PID_CGROUP: u64 = 1 << 2;
    const PERF_FLAG_FD_CLOEXEC: u64 = 1 << 3;

    let valid_flags =
        PERF_FLAG_FD_NO_GROUP | PERF_FLAG_FD_OUTPUT | PERF_FLAG_PID_CGROUP | PERF_FLAG_FD_CLOEXEC;
    if flags & !valid_flags != 0 {
        return Err(SysError::EINVAL);
    }

    let addr_space = task.addr_space();
    let mut attr_ptr = UserReadPtr::<u8>::new(attr_ptr, &addr_space);

    let size_bytes = unsafe { attr_ptr.read_array(8) }?; // type(4) + size(4)
    let attr_size =
        u32::from_ne_bytes([size_bytes[4], size_bytes[5], size_bytes[6], size_bytes[7]]);

    if attr_size < PERF_ATTR_SIZE_VER0 || attr_size > core::mem::size_of::<PerfEventAttr>() as u32 {
        return Err(SysError::E2BIG);
    }

    let attr_bytes = unsafe { attr_ptr.read_array(attr_size as usize) }?;
    let mut attr = PerfEventAttr::new();

    // type (u32) + size (u32) = 8 bytes
    if attr_size >= 8 {
        attr.r#type =
            u32::from_ne_bytes([attr_bytes[0], attr_bytes[1], attr_bytes[2], attr_bytes[3]]);
        attr.size =
            u32::from_ne_bytes([attr_bytes[4], attr_bytes[5], attr_bytes[6], attr_bytes[7]]);
    }

    // config (u64) = 8 bytes, offset 8
    if attr_size >= 16 {
        attr.config = u64::from_ne_bytes([
            attr_bytes[8],
            attr_bytes[9],
            attr_bytes[10],
            attr_bytes[11],
            attr_bytes[12],
            attr_bytes[13],
            attr_bytes[14],
            attr_bytes[15],
        ]);
    }

    // sample_period_freq (u64) = 8 bytes, offset 16
    if attr_size >= 24 {
        attr.sample_period_freq = u64::from_ne_bytes([
            attr_bytes[16],
            attr_bytes[17],
            attr_bytes[18],
            attr_bytes[19],
            attr_bytes[20],
            attr_bytes[21],
            attr_bytes[22],
            attr_bytes[23],
        ]);
    }

    // sample_type (u64) = 8 bytes, offset 24
    if attr_size >= 32 {
        attr.sample_type = u64::from_ne_bytes([
            attr_bytes[24],
            attr_bytes[25],
            attr_bytes[26],
            attr_bytes[27],
            attr_bytes[28],
            attr_bytes[29],
            attr_bytes[30],
            attr_bytes[31],
        ]);
    }

    // read_format (u64) = 8 bytes, offset 32
    if attr_size >= 40 {
        attr.read_format = u64::from_ne_bytes([
            attr_bytes[32],
            attr_bytes[33],
            attr_bytes[34],
            attr_bytes[35],
            attr_bytes[36],
            attr_bytes[37],
            attr_bytes[38],
            attr_bytes[39],
        ]);
    }

    // flags (u64) = 8 bytes, offset 40
    if attr_size >= 48 {
        attr.flags = u64::from_ne_bytes([
            attr_bytes[40],
            attr_bytes[41],
            attr_bytes[42],
            attr_bytes[43],
            attr_bytes[44],
            attr_bytes[45],
            attr_bytes[46],
            attr_bytes[47],
        ]);
    }

    // wakeup_events_watermark (u32) + bp_type (u32) = 8 bytes, offset 48
    if attr_size >= 56 {
        attr.wakeup_events_watermark = u32::from_ne_bytes([
            attr_bytes[48],
            attr_bytes[49],
            attr_bytes[50],
            attr_bytes[51],
        ]);
        attr.bp_type = u32::from_ne_bytes([
            attr_bytes[52],
            attr_bytes[53],
            attr_bytes[54],
            attr_bytes[55],
        ]);
    }

    // bp_addr_config1 (u64) = 8 bytes, offset 56
    if attr_size >= 64 {
        attr.bp_addr_config1 = u64::from_ne_bytes([
            attr_bytes[56],
            attr_bytes[57],
            attr_bytes[58],
            attr_bytes[59],
            attr_bytes[60],
            attr_bytes[61],
            attr_bytes[62],
            attr_bytes[63],
        ]);
    }

    // bp_len_config2 (u64) = 8 bytes, offset 64
    if attr_size >= 72 {
        attr.bp_len_config2 = u64::from_ne_bytes([
            attr_bytes[64],
            attr_bytes[65],
            attr_bytes[66],
            attr_bytes[67],
            attr_bytes[68],
            attr_bytes[69],
            attr_bytes[70],
            attr_bytes[71],
        ]);
    }

    // config3 (u64) = 8 bytes, offset 72
    if attr_size >= 80 {
        attr.config3 = u64::from_ne_bytes([
            attr_bytes[72],
            attr_bytes[73],
            attr_bytes[74],
            attr_bytes[75],
            attr_bytes[76],
            attr_bytes[77],
            attr_bytes[78],
            attr_bytes[79],
        ]);
    }

    // branch_sample_type (u64) = 8 bytes, offset 80
    if attr_size >= 88 {
        attr.branch_sample_type = u64::from_ne_bytes([
            attr_bytes[80],
            attr_bytes[81],
            attr_bytes[82],
            attr_bytes[83],
            attr_bytes[84],
            attr_bytes[85],
            attr_bytes[86],
            attr_bytes[87],
        ]);
    }

    // sample_regs_user (u64) = 8 bytes, offset 88
    if attr_size >= 96 {
        attr.sample_regs_user = u64::from_ne_bytes([
            attr_bytes[88],
            attr_bytes[89],
            attr_bytes[90],
            attr_bytes[91],
            attr_bytes[92],
            attr_bytes[93],
            attr_bytes[94],
            attr_bytes[95],
        ]);
    }

    // sample_stack_user (u32) + clockid (i32) = 8 bytes, offset 96
    if attr_size >= 104 {
        attr.sample_stack_user = u32::from_ne_bytes([
            attr_bytes[96],
            attr_bytes[97],
            attr_bytes[98],
            attr_bytes[99],
        ]);
        attr.clockid = i32::from_ne_bytes([
            attr_bytes[100],
            attr_bytes[101],
            attr_bytes[102],
            attr_bytes[103],
        ]);
    }

    // sample_regs_intr (u64) = 8 bytes, offset 104
    if attr_size >= 112 {
        attr.sample_regs_intr = u64::from_ne_bytes([
            attr_bytes[104],
            attr_bytes[105],
            attr_bytes[106],
            attr_bytes[107],
            attr_bytes[108],
            attr_bytes[109],
            attr_bytes[110],
            attr_bytes[111],
        ]);
    }

    // aux_watermark (u32) + sample_max_stack (u16) + __reserved_2 (u16) = 8 bytes, offset 112
    if attr_size >= 120 {
        attr.aux_watermark = u32::from_ne_bytes([
            attr_bytes[112],
            attr_bytes[113],
            attr_bytes[114],
            attr_bytes[115],
        ]);
        attr.sample_max_stack = u16::from_ne_bytes([attr_bytes[116], attr_bytes[117]]);
        attr.__reserved_2 = u16::from_ne_bytes([attr_bytes[118], attr_bytes[119]]);
    }

    // aux_sample_size (u32) + __reserved_3 (u32) = 8 bytes, offset 120
    if attr_size >= 128 {
        attr.aux_sample_size = u32::from_ne_bytes([
            attr_bytes[120],
            attr_bytes[121],
            attr_bytes[122],
            attr_bytes[123],
        ]);
        attr.__reserved_3 = u32::from_ne_bytes([
            attr_bytes[124],
            attr_bytes[125],
            attr_bytes[126],
            attr_bytes[127],
        ]);
    }

    // sig_data (u64) = 8 bytes, offset 128
    if attr_size >= 136 {
        attr.sig_data = u64::from_ne_bytes([
            attr_bytes[128],
            attr_bytes[129],
            attr_bytes[130],
            attr_bytes[131],
            attr_bytes[132],
            attr_bytes[133],
            attr_bytes[134],
            attr_bytes[135],
        ]);
    }

    if !attr.validate() {
        return Err(SysError::EINVAL);
    }

    let target_pid = if pid == 0 {
        task.pid() as i32
    } else if pid == -1 {
        -1
    } else if pid > 0 {
        // todo! check process perm (EPERM)
        pid
    } else {
        return Err(SysError::EINVAL);
    };

    let target_cpu = if cpu == -1 {
        -1
    } else if cpu >= 0 {
        // todo! check cpu valid (EINVAL)
        cpu
    } else {
        return Err(SysError::EINVAL);
    };

    let mut group_leader: Option<Arc<PerfEventFile>> = None;
    if group_fd >= 0 {
        if flags & PERF_FLAG_FD_NO_GROUP != 0 {
            return Err(SysError::EINVAL);
        }

        let group_file = task.with_mut_fdtable(|table| table.get_file(group_fd as usize))?;
        let perf_group_file = group_file
            .as_any()
            .downcast_ref::<PerfEventFile>()
            .ok_or(SysError::EBADF)?;

        let group_attr = perf_group_file.get_attr()?;
        if group_attr.r#type != attr.r#type {
            return Err(SysError::EINVAL);
        }

        group_leader = Some(
            group_file
                .downcast_arc::<PerfEventFile>()
                .map_err(|_| SysError::EINVAL)?,
        );
    }

    fn is_perf_paranoid_allowed(level: i32) -> bool {
        // todo!: check perf_event_paranoid setup
        // 0: 不限制
        // 1: 限制 CPU 事件和内核分析
        // 2: 限制内核分析
        // 3: 禁用所有
        let paranoid_level = get_perf_event_paranoid();
        paranoid_level <= level
    }

    fn get_perf_event_paranoid() -> i32 {
        // todo!: read /proc/sys/kernel/perf_event_paranoid
        // default strict setup
        2
    }

    match PerfType::try_from_u32(attr.r#type) {
        Ok(PerfType::Hardware) | Ok(PerfType::HwCache) => {
            // Hareware events need 1 perm
            if !task.has_capability(CapabilitiesFlags::CAP_SYS_ADMIN)
                && !is_perf_paranoid_allowed(1)
            {
                return Err(SysError::EACCES);
            }
        }
        Ok(PerfType::Tracepoint) => {
            // Tracepoint events need 0 perm
            if !task.has_capability(CapabilitiesFlags::CAP_SYS_ADMIN)
                && !is_perf_paranoid_allowed(0)
            {
                return Err(SysError::EACCES);
            }
        }
        Ok(PerfType::Raw) => {
            // Raw events need CAP_SYS_ADMIN
            if !task.has_capability(CapabilitiesFlags::CAP_SYS_ADMIN) {
                return Err(SysError::EACCES);
            }
        }
        _ => {} // Software Events are always allowed
    }

    // allocate unique event id
    static EVENT_ID_COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
    let event_id = EVENT_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    // create perf event
    let perf_inode = PerfEventInode::new(attr.clone(), target_pid, target_cpu, group_fd, event_id);
    perf_inode.set_mode(config::inode::InodeMode::REG);

    // create dentry and file
    let dentry = SimpleDentry::new(
        "perf_event",
        Some(perf_inode.clone()),
        Some(Arc::downgrade(&sys_root_dentry())),
    );
    sys_root_dentry().add_child(dentry.clone());

    let perf_file = PerfEventFile::new(dentry);

    if let Some(leader) = group_leader {
        perf_file.set_group_leader(Some(leader.clone()))?;
        leader.add_group_member(Arc::downgrade(&perf_file))?;
        perf_file.sync_with_leader(&leader)?;
    }

    perf_file.setup()?;

    osfs::special::perf::file::register_perf_event(&perf_file);

    let mut file_flags = OpenFlags::O_RDONLY;
    if flags & PERF_FLAG_FD_CLOEXEC != 0 {
        file_flags |= OpenFlags::O_CLOEXEC;
    }

    let fd = task.with_mut_fdtable(|ft| ft.alloc(perf_file, file_flags))?;
    log::debug!(
        "[sys_perf_event_open] created event fd={}, id={}, type={}",
        fd,
        event_id,
        attr.r#type
    );

    Ok(fd)
}

#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum IdType {
    /// Wait for any child
    P_ALL = 0,
    /// Wait for specific PID
    P_PID = 1,
    /// Wait for any child in the same process group
    P_PGID = 2,
    /// Wait for any child in the specified process group
    P_PIDFD = 3,
}

/// The `waitid` system call waits for a child process to change state,
/// and optionally retrieves information about the child whose state has changed.
///
/// # Arguments
/// - `idtype`: Specifies which children to wait for
/// - `id`: Specifies the child(ren) to wait for, as determined by `idtype`
/// - `infop`: Used to return information about the child
/// - `options`: Controls the behavior of the call
///
/// # Note
/// Similar to `wait4`, but provides more control and returns siginfo_t structure
pub async fn sys_waitid(idtype: i32, id: i32, infop: usize, options: i32) -> SyscallResult {
    let task = current_task();
    log::info!("[sys_waitid] {} wait for recycling", task.get_name());

    let idtype = IdType::from_repr(idtype).ok_or(SysError::EINVAL)?;
    let option = WaitIdOptions::from_bits(options).ok_or(SysError::EINVAL)?;

    log::info!("[sys_waitid] idtype: {idtype:?}, id: {id}, option: {option:?}");

    // Determine what to wait for based on idtype and id
    let target = match idtype {
        IdType::P_ALL => WaitFor::AnyChild,
        IdType::P_PID => {
            if id <= 0 {
                return Err(SysError::EINVAL);
            }
            WaitFor::Pid(id as Pid)
        }
        IdType::P_PGID => {
            if id < 0 {
                return Err(SysError::EINVAL);
            }
            if id == 0 {
                WaitFor::AnyChildInGroup
            } else {
                WaitFor::PGid(id as PGid)
            }
        }
        IdType::P_PIDFD => {
            // P_PIDFD is not implemented yet
            return Err(SysError::ENOSYS);
        }
    };

    // Check if we should report exited children
    let report_exited = option.contains(WaitIdOptions::WEXITED);
    // Check if we should report stopped children
    let report_stopped = option.contains(WaitIdOptions::WSTOPPED);
    // Check if we should report continued children
    let report_continued = option.contains(WaitIdOptions::WCONTINUED);

    if !report_exited && !report_stopped && !report_continued {
        return Err(SysError::EINVAL);
    }

    // Get the child for recycle according to the target
    let child_for_recycle = match target {
        WaitFor::AnyChild => {
            let children = task.children_mut().lock();
            if children.is_empty() {
                log::info!("[sys_waitid] task [{}] fail: no child", task.get_name());
                return Err(SysError::ECHILD);
            }
            children
                .values()
                .find(|c| {
                    if report_exited && c.is_in_state(TaskState::WaitForRecycle) {
                        true
                    } else if report_stopped && c.is_in_state(TaskState::Sleeping) {
                        true
                    } else {
                        false
                    }
                })
                .cloned()
        }
        WaitFor::Pid(pid) => {
            let children = task.children_mut().lock();
            if children.is_empty() {
                log::info!("[sys_waitid] task [{}] fail: no child", task.get_name());
                return Err(SysError::ECHILD);
            }
            if let Some(child) = children.get(&pid) {
                if (report_exited && child.is_in_state(TaskState::WaitForRecycle))
                    || (report_stopped && child.is_in_state(TaskState::Sleeping))
                {
                    Some(child.clone())
                } else {
                    None
                }
            } else {
                log::info!("[sys_waitid] fail: no child with pid {pid}");
                return Err(SysError::ECHILD);
            }
        }
        WaitFor::PGid(pgid) => {
            let mut result = None;
            for process in PROCESS_GROUP_MANAGER
                .get_group(pgid)
                .ok_or(SysError::ECHILD)?
                .into_iter()
                .filter_map(|t| t.upgrade())
                .filter(|t| t.is_process())
            {
                let children = process.children_mut().lock();
                if let Some(child) = children
                    .values()
                    .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                {
                    result = Some(child.clone());
                    break;
                }
            }
            result
        }
        WaitFor::AnyChildInGroup => {
            let pgid = task.get_pgid();
            let mut result = None;
            for process in PROCESS_GROUP_MANAGER
                .get_group(pgid)
                .ok_or(SysError::ECHILD)?
                .into_iter()
                .filter_map(|t| t.upgrade())
                .filter(|t| t.is_process())
            {
                let children = process.children_mut().lock();
                if let Some(child) = children
                    .values()
                    .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                {
                    result = Some(child.clone());
                    break;
                }
            }
            result
        }
    };

    if let Some(child_for_recycle) = child_for_recycle {
        // If there is a child for recycle when `sys_waitid` is called
        let addr_space = task.addr_space();
        let zombie_task = child_for_recycle;

        // Update child time
        task.timer_mut().update_child_time((
            zombie_task.timer_mut().user_time(),
            zombie_task.timer_mut().kernel_time(),
        ));

        // Fill siginfo structure if infop is not null
        if infop != 0 {
            let mut siginfo_ptr = UserWritePtr::<LinuxSigInfo>::new(infop, &addr_space);

            let (si_code, si_status) = if zombie_task.is_in_state(TaskState::Sleeping) {
                // Stopped child
                (SigInfo::CLD_STOPPED, signal::Sig::SIGSTOP.raw() as i32)
            } else {
                // Exited or killed child
                let exit_code = zombie_task.get_exit_code();
                if exit_code & 0x7F == 0 {
                    // Normal exit: status is in high 8 bits
                    (SigInfo::CLD_EXITED, (exit_code >> 8) & 0xFF)
                } else {
                    // Killed by signal: signal number is in low 7 bits
                    (SigInfo::CLD_KILLED, exit_code & 0x7F)
                }
            };

            let siginfo = LinuxSigInfo {
                si_signo: signal::Sig::SIGCHLD.raw() as i32,
                si_errno: 0,
                si_code,
                si_pid: zombie_task.tid() as i32,
                si_uid: zombie_task.uid() as u32,
                si_status,
                si_utime: zombie_task.timer_mut().user_time().as_micros() as u32,
                si_stime: zombie_task.timer_mut().kernel_time().as_micros() as u32,
                ..Default::default()
            };

            unsafe {
                siginfo_ptr.write(siginfo)?;
            }
        }

        let tid = zombie_task.tid();
        log::debug!(
            "[sys_waitid] remove tid [{}] task [{}]",
            tid,
            zombie_task.get_name()
        );

        // Don't leave child in a waitable state if WNOWAIT is not set
        // Also, don't remove stopped children unless they exit
        if !option.contains(WaitIdOptions::WNOWAIT) && !zombie_task.is_in_state(TaskState::Sleeping)
        {
            task.remove_child(zombie_task.clone());
            TASK_MANAGER.remove_task(tid);
            PROCESS_GROUP_MANAGER.remove(&zombie_task);
        }

        Ok(0)
    } else if option.contains(WaitIdOptions::WNOHANG) {
        // If WNOHANG option is set and there is no child for recycle, return immediately
        log::debug!("[sys_waitid] WaitIdOptions::WNOHANG return");

        // Clear siginfo if provided
        if infop != 0 {
            let addr_space = task.addr_space();
            let mut siginfo_ptr = UserWritePtr::<LinuxSigInfo>::new(infop, &addr_space);
            let empty_siginfo = LinuxSigInfo::default();
            unsafe {
                siginfo_ptr.write(empty_siginfo)?;
            }
        }

        Ok(0)
    } else {
        WAIT_QUEUE_MANAGER.add_waiter(task.clone(), target.clone());

        log::info!(
            "[sys_waitid] task [{}] suspend using wait queue for target: {:?}",
            task.get_name(),
            target
        );

        task.set_state(TaskState::Interruptible);
        suspend_now().await;
        task.set_state(TaskState::Running);

        let (child_tid, exit_code, child_utime, child_stime) = {
            // check if there is a child for recycle
            // NOTE: no loop here, only continue waiting if user set SA_RESTART or loop call `sys_wait4`
            let child = match target {
                WaitFor::AnyChild => {
                    let children = task.children_mut().lock();
                    children
                        .values()
                        .find(|c| {
                            c.is_in_state(TaskState::WaitForRecycle)
                                && c.with_thread_group(|tg| tg.len() == 1)
                        })
                        .cloned()
                }
                WaitFor::Pid(pid) => {
                    let children = task.children_mut().lock();
                    if let Some(child) = children.get(&pid) {
                        if child.is_in_state(TaskState::WaitForRecycle)
                            && child.with_thread_group(|tg| tg.len() == 1)
                        {
                            Some(child.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                WaitFor::PGid(pgid) => {
                    let mut result = None;
                    for process in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                        .filter(|t| t.is_process())
                    {
                        log::info!(
                            "[sys_wait4] in PGid block, task {} try to find the assigned child task after suspending(target: {:?})",
                            task.tid(),
                            target
                        );
                        let children = process.children_mut().lock();
                        if let Some(child) = children
                            .values()
                            .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                        {
                            result = Some(child.clone());
                            break;
                        }
                    }
                    result
                }
                WaitFor::AnyChildInGroup => {
                    log::info!(
                        "[sys_wait4] in AnyChildInGroup block, task {} try to find the assigned child task after suspending(target: {:?})",
                        task.tid(),
                        target
                    );
                    let pgid = task.get_pgid();
                    let mut result = None;

                    let btree = PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?;
                    log::info!("[sys_wait4] pgid {} group length: {}", pgid, btree.len());
                    for task in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                    {
                        log::debug!("[sys_wait4] task {} in pgid {} group", task.tid(), pgid);
                    }

                    for process in PROCESS_GROUP_MANAGER
                        .get_group(pgid)
                        .ok_or(SysError::ECHILD)?
                        .into_iter()
                        .filter_map(|t| t.upgrade())
                        .filter(|t| t.is_process())
                    {
                        let children = process.children_mut().lock();
                        if let Some(child) = children
                            .values()
                            .find(|c| c.is_in_state(TaskState::WaitForRecycle))
                        {
                            result = Some(child.clone());
                            break;
                        }
                    }
                    result
                }
            };

            if let Some(child) = child {
                log::info!(
                    "[sys_wait4] task {} found the child task {} for recycle after suspending",
                    task.tid(),
                    child.tid()
                );
                // 从等待队列中移除当前任务
                WAIT_QUEUE_MANAGER.remove_waiter(&task);
                (
                    child.tid(),
                    child.get_exit_code(),
                    child.timer_mut().user_time(),
                    child.timer_mut().kernel_time(),
                )
            } else {
                // 检查是否被信号中断
                if task
                    .sig_manager_mut()
                    .has_expect_signals(!*task.sig_mask_mut())
                {
                    log::info!("[sys_wait4] task {} interrupted by signal", task.tid());
                    WAIT_QUEUE_MANAGER.remove_waiter(&task);
                }
                log::debug!(
                    "[sys_wait4] task {} will continue waiting if SA_RESTART is set",
                    task.tid()
                );
                return Err(SysError::EINTR);
            }
        };

        // Update child time
        task.timer_mut()
            .update_child_time((child_utime, child_stime));

        // Fill siginfo structure if infop is not null
        if infop != 0 {
            let addr_space = task.addr_space();
            let mut siginfo_ptr = UserWritePtr::<LinuxSigInfo>::new(infop, &addr_space);

            // Decode the exit status: for normal exit, extract the original status from high 8 bits
            // For signal kill, it's stored in low 7 bits
            let (si_code, si_status) = if exit_code & 0x7F == 0 {
                // Normal exit: status is in high 8 bits
                (SigInfo::CLD_EXITED, (exit_code >> 8) & 0xFF)
            } else {
                // Killed by signal: signal number is in low 7 bits
                (SigInfo::CLD_KILLED, exit_code & 0x7F)
            };

            let siginfo = LinuxSigInfo {
                si_signo: signal::Sig::SIGCHLD.raw() as i32,
                si_errno: 0,
                si_code,
                si_pid: child_tid as i32,
                si_uid: 0, // We don't have uid info for the child here
                si_status,
                si_utime: child_utime.as_micros() as u32,
                si_stime: child_stime.as_micros() as u32,
                ..Default::default()
            };

            unsafe {
                siginfo_ptr.write(siginfo)?;
            }
        }

        // Check if the child is still in TASK_MANAGER
        let child = TASK_MANAGER.get_task(child_tid).unwrap();
        log::info!(
            "[sys_waitid] remove task [{}] with tid [{}]",
            child_tid,
            child.get_name()
        );

        // Don't leave child in a waitable state if WNOWAIT is not set
        if !option.contains(WaitIdOptions::WNOWAIT) {
            // Remove the child from current task's children, and TASK_MANAGER
            PROCESS_GROUP_MANAGER.remove(&child);
            task.remove_child(child);
            TASK_MANAGER.remove_task(child_tid);
        }

        Ok(0)
    }
}
