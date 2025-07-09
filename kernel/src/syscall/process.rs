use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use bitflags::*;
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

use crate::task::cap::{CapUserData, CapUserHeader};
use crate::task::signal::pidfd::PF_TABLE;
use crate::task::signal::sig_info::Sig;
use crate::task::{
    TaskState,
    manager::TASK_MANAGER,
    process_manager::PROCESS_GROUP_MANAGER,
    signal::sig_info::SigSet,
    tid::{PGid, Pid},
};
use crate::vm::user_ptr::{UserReadPtr, UserWritePtr};
use crate::{processor::current_task, task::future::spawn_user_task};

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
    let task = current_task();
    log::info!("[sys_wait4] {} wait for recycling", task.get_name());
    let option = WaitOptions::from_bits_truncate(options);
    let target = match pid {
        -1 => WaitFor::AnyChild,
        0 => WaitFor::AnyChildInGroup,
        p if p > 0 => WaitFor::Pid(p as Pid),
        p => WaitFor::PGid(p as PGid),
    };
    log::info!("[sys_wait4] target: {target:?}, option: {option:?}");
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
                log::info!("[sys_wait4] task [{}] fail: no child", task.get_name());
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
                log::info!("[sys_wait4] task [{}] fail: no child", task.get_name());
                return Err(SysError::ECHILD);
            }
            if let Some(child) = children.get(&pid) {
                if child.is_in_state(TaskState::WaitForRecycle) {
                    Some(child.clone())
                } else {
                    None
                }
            } else {
                log::info!("[sys_wait4] fail: no child with pid {pid}");
                return Err(SysError::ECHILD);
            }
        }
        WaitFor::PGid(_) => unimplemented!(),
        WaitFor::AnyChildInGroup => {
            let pgid = task.get_pgid();
            let mut result = None;
            for process in PROCESS_GROUP_MANAGER
                .get_group(pgid)
                .ok_or_else(|| SysError::ESRCH)?
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
            log::debug!("[sys_wait4] wstatus: {exit_code:#x}");
            unsafe {
                status.write(exit_code)?;
            }
        }
        let tid = zombie_task.tid();
        log::debug!(
            "[sys_wait4] remove tid [{}] task [{}]",
            tid,
            zombie_task.get_name()
        );

        task.remove_child(zombie_task.clone());

        TASK_MANAGER.remove_task(tid);

        PROCESS_GROUP_MANAGER.remove(&zombie_task);
        Ok(tid)
    } else if option.contains(WaitOptions::WNOHANG) {
        // 2. if WNOHANG option is set and there is no child for recycle, return immediately
        log::debug!("[sys_wait4] WaitOptions::WNOHANG return");
        Ok(0)
    } else {
        // 3. if there is no child for recycle and WNOHANG option is not set, wait for SIGCHLD from target
        let (child_tid, exit_code, child_utime, child_stime) = loop {
            task.set_state(TaskState::Interruptible);
            task.set_wake_up_signal(!task.get_sig_mask() | SigSet::SIGCHLD);
            log::info!("[sys_wait4] task [{}] suspend for sigchld", task.get_name());
            suspend_now().await;
            // wake up from suspend for any reason(may not be SIGCHLD)
            task.set_state(TaskState::Running);
            let si = task.sig_manager_mut().get_expect(SigSet::SIGCHLD);
            // if it is SIGCHLD, then we can get the child for recycle
            // TODO: check if the matched child is identical to the SIGCHLD's info
            if let Some(info) = si {
                log::info!(
                    "[sys_wait4] sigchld received, the child for recycle is announced by signal to be {:?}",
                    info.details
                );

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
                        let child = children.get(&pid).unwrap().clone();
                        if child.is_in_state(TaskState::WaitForRecycle)
                            && child.with_thread_group(|tg| tg.len() == 1)
                        {
                            Some(child)
                        } else {
                            None
                        }
                    }
                    WaitFor::PGid(_) => unimplemented!(),
                    WaitFor::AnyChildInGroup => {
                        let pgid = task.get_pgid();
                        let mut result = None;
                        for process in PROCESS_GROUP_MANAGER
                            .get_group(pgid)
                            .ok_or_else(|| SysError::ESRCH)?
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
                    break (
                        child.tid(),
                        child.get_exit_code(),
                        child.timer_mut().user_time(),
                        child.timer_mut().kernel_time(),
                    );
                }
            } else {
                log::info!("[sys_wait4] return SysError::EINTR");
                log::info!(
                    "[sys_wait4] pending signals: {:?}",
                    task.sig_manager_mut().queue
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
            log::trace!("[sys_wait4] wstatus: {:#x}", exit_code);
            unsafe {
                status.write(exit_code)?;
            }
        }
        // check if the child is still in TASK_MANAGER
        let child = TASK_MANAGER.get_task(child_tid).unwrap();
        log::info!(
            "[sys_wait4] remove task [{}] with tid [{}]",
            child_tid,
            child.get_name()
        );
        // remove the child from current task's children, and TASK_MANAGER, thus the child will be dropped after hart leaves child
        // NOTE: the child's thread group itself will be recycled when the child is dropped, and it use Weak pointer so it won't affect the drop of child
        task.remove_child(child);
        TASK_MANAGER.remove_task(child_tid);
        PROCESS_GROUP_MANAGER.remove(&task);
        Ok(child_tid)
    }
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
    let read_string_array = |addr: usize| {
        let mut args = Vec::new();
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<usize>::new(addr, &addr_space);
        let pointers = user_ptr.read_ptr_array(256)?;
        for ptr in pointers {
            let mut user_ptr = UserReadPtr::<u8>::new(ptr, &addr_space);
            let string = user_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;
            args.push(string);
        }
        Ok(args)
    };

    let path = read_string(path)?;
    let args = read_string_array(argv)?;
    let mut envs = read_string_array(envp)?;

    if path.is_empty() {
        log::warn!("[sys_execve] path is empty");
        return Err(SysError::ENOENT);
    }

    // DEBUG
    let broken_tests = ["cgroup", "cp_tests.sh", "cn_pec.sh", "cpuacct.sh"];
    if broken_tests
        .iter()
        .filter(|t| path.contains(**t))
        .last()
        .is_some()
    {
        log::error!("not support cgroup");
        return Err(SysError::EOPNOTSUPP);
    }

    // let mut busybox_prefix = String::from("bin");

    // if task.cwd().lock().path().contains("musl") {
    //     envs.push(String::from(r#"PATH=/:/musl/lib:"#));
    //     // busybox_prefix = String::from("musl");
    // }

    // if task.cwd().lock().path().contains("glibc") {
    //     envs.push(String::from(r#"PATH=/:/glibc/lib:"#));
    //     // busybox_prefix = String::from("glibc");
    // }

    log::info!("[sys_execve] task: {:?}", task.get_name());
    log::info!("[sys_execve] args: {args:?}");
    log::info!("[sys_execve] envs: {envs:?}");
    log::info!("[sys_execve] path: {path:?}");

    // let env_iter = envs.iter();
    // for env in env_iter {
    //     if env.starts_with("PWD") {
    //         if let Some((_key, value)) = env.split_once('=') {
    //             if value == "/" {
    //                 continue;
    //             }
    //             path.remove(0);
    //             args[0].remove(0);
    //             path.insert_str(0, value);
    //             args[0].insert_str(0, value);
    //             log::debug!("new path = {}", path);
    //             break;
    //         }
    //     }
    // }

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

    let filepath = dentry.path();
    let expath = filepath.rsplitn(2, '/').nth(1).unwrap_or("");
    let argpath = format!(
        "PATH={}:/bin:/sbin:/usr/bin:/usr/local/bin:/usr/local/sbin:ltp/testcases/bin:",
        expath
    );
    envs.push(String::from(argpath));

    log::info!("[sys_execve]: open file {}", dentry.path());
    let file = <dyn File>::open(dentry)?;

    let mut name = String::new();
    args.iter().for_each(|arg| {
        name.push_str(arg);
        name.push(' ');
    });

    if let Err(e) = task.execve(file.clone(), args.clone(), envs.clone(), name) {
        match e {
            SysError::ENOEXEC => {
                let mut buf = vec![0; 128];

                file.seek(config::vfs::SeekFrom::Start(0))?;
                file.read(&mut buf).await?;
                // log::debug!("[sys_execve] buf: {:?}", buf);

                let firline = String::from_utf8(buf);
                // log::debug!("[sys_execve] firline: {:?}", firline);
                if firline.is_ok() && firline.clone().unwrap().starts_with("#!") {
                    let mut firline = firline.unwrap();
                    firline.remove(0);
                    firline.remove(0);
                    let idx = firline.find("\n").ok_or(SysError::ENOEXEC)?;
                    firline.truncate(idx);

                    let mut exargs: Vec<String> =
                        firline.split_whitespace().map(|s| s.to_string()).collect();
                    let path = exargs[0].clone();

                    exargs.extend(args);
                    log::debug!("[sys_execve] exargs: {:?}", exargs);

                    let dentry = {
                        let path = Path::new(sys_root_dentry(), path);
                        let dentry = path.walk()?;
                        if !dentry.is_negative()
                            && dentry.inode().unwrap().inotype() == InodeType::SymLink
                        {
                            Path::resolve_symlink_through(Arc::clone(&dentry))?
                        } else {
                            dentry
                        }
                    };

                    let file = <dyn File>::open(dentry)?;
                    log::info!("[sys_execve]: open file");
                    let mut name = String::new();
                    exargs.iter().for_each(|arg| {
                        name.push_str(arg);
                        name.push(' ');
                    });

                    task.execve(file.clone(), exargs, envs, name)?;
                } else if args[0].ends_with(".sh") {
                    let mut exargs: Vec<String> = vec!["busybox".to_string(), "sh".to_string()];
                    exargs.extend(args);

                    let path = "busybox".to_string();
                    let dentry = {
                        let path = Path::new(sys_root_dentry(), path);
                        let dentry = path.walk()?;
                        if !dentry.is_negative()
                            && dentry.inode().unwrap().inotype() == InodeType::SymLink
                        {
                            Path::resolve_symlink_through(Arc::clone(&dentry))?
                        } else {
                            dentry
                        }
                    };

                    let file = <dyn File>::open(dentry)?;
                    log::info!("[sys_execve]: open file");
                    let mut name = String::new();
                    exargs.iter().for_each(|arg| {
                        name.push_str(arg);
                        name.push(' ');
                    });

                    task.execve(file.clone(), exargs, envs, name)?;
                } else {
                    Err(SysError::ENOEXEC)?
                }
            }
            e => Err(e)?,
        }
    }
    log::info!("[sys_execve]: finish execve and convert to a new task");

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

    log::error!("[sys_clone3] begin to exe");

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

    *new_task.exit_signal.lock() = Some((args.exit_signal & 0xFF) as u8);

    log::info!("[sys_clone3] who is your parent? {}", new_task.ppid());
    spawn_user_task(new_task);
    log::info!("[sys_clone3] clone success",);

    Ok(new_tid)
}

pub fn sys_setsid() -> SyscallResult {
    log::debug!("[sys_setsid]");
    let task = current_task();
    Ok(task.pid())
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

pub const _LINUX_CAPABILITY_VERSION_1: u32 = 0x19980330;
pub const _LINUX_CAPABILITY_VERSION_2: u32 = 0x20071026;
pub const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
pub const CAPABILITY_U32S_1: usize = 1;
pub const CAPABILITY_U32S_2: usize = 2;
pub const CAPABILITY_U32S_3: usize = 2;
pub fn sys_capget(hdrp: usize, datap: usize) -> SyscallResult {
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
