use crate::task::TaskState;
use crate::task::signal::sig_info::SigSet;
use crate::task::{
    manager::TASK_MANAGER,
    process_manager::PROCESS_GROUP_MANAGER,
    tid::{PGid, Pid},
};
use crate::vm::user_ptr::{UserReadPtr, UserWritePtr};
use crate::{processor::current_task, task::future::spawn_user_task};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use config::inode::InodeType;
use config::mm::USER_STACK_SIZE;
use config::process::CloneFlags;
use osfs::sys_root_dentry;
use osfuture::{suspend_now, yield_now};
use strum::FromRepr;
use systype::{RLimit, SysError, SyscallResult};
use vfs::file::File;
use vfs::path::Path;

/// `gettid` returns the caller's thread ID (TID).  
///
/// # Type
/// - In a single-threaded process, the thread ID is equal to the process ID (PID, as returned by getpid(2)).
/// - In a multi-threaded process, all threads have the same PID, but each one has a unique TID.
pub fn sys_gettid() -> SyscallResult {
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
    log::info!("[sys_getppid] ppid: {r:?}");
    Ok(r)
}

/// `exit()` system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are performed
/// only if this is the last thread in the thread group.
pub fn sys_exit(status: i32) -> SyscallResult {
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

    // get the child for recycle according to the target
    // NOTE: recycle no more than one child per `sys_wait4`
    let child_for_recycle = {
        let children = task.children_mut().lock();
        if children.is_empty() {
            log::info!("[sys_wait4] task [{}] fail: no child", task.get_name());
            return Err(SysError::ECHILD);
        }
        // TODO: PGid and AnyChildInGroup targets
        match target {
            WaitFor::AnyChild => children
                .values()
                .find(|c| c.is_in_state(TaskState::WaitForRecycle)),
            WaitFor::Pid(pid) => {
                if let Some(child) = children.get(&pid) {
                    if child.is_in_state(TaskState::WaitForRecycle) {
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
        log::error!(
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
        Ok(0)
    } else {
        // 3. if there is no child for recycle and WNOHANG option is not set, wait for SIGCHLD from target
        log::info!("[sys_wait4] task [{}] suspend for sigchld", task.get_name());
        let (child_tid, exit_code, child_utime, child_stime) = loop {
            task.set_state(TaskState::Interruptable);
            task.set_wake_up_signal(!task.get_sig_mask() | SigSet::SIGCHLD);
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
                let children = task.children_mut().lock();

                let child = match target {
                    WaitFor::AnyChild => children.values().find(|c| {
                        c.is_in_state(TaskState::WaitForRecycle)
                            && c.with_thread_group(|tg| tg.len() == 1)
                    }),

                    WaitFor::Pid(pid) => {
                        let child = children.get(&pid).unwrap();
                        if child.is_in_state(TaskState::WaitForRecycle)
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
                        child.timer_mut().kernel_time(),
                    );
                }
            } else {
                log::info!("return SysError::EINTR");
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
pub fn sys_clone(
    flags: usize,
    stack: usize,
    parent_tid_ptr: usize,
    tls_ptr: usize,
    chilren_tid_ptr: usize,
) -> SyscallResult {
    log::info!(
        "[sys_clone] flags:{flags:?}, stack:{stack:#x}, tls:{tls_ptr:#x}, parent_tid:{parent_tid_ptr:#x}, child_tid:{chilren_tid_ptr:x}"
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
        unsafe { parent_tid.write(new_tid)? };
    }
    if flags.contains(CloneFlags::CHILD_SETTID) {
        let mut chilren_tid = UserWritePtr::<usize>::new(chilren_tid_ptr, &addrspace);
        unsafe { chilren_tid.write(new_tid)? };
        new_task.tid_address_mut().set_child_tid = Some(chilren_tid_ptr);
    }
    if flags.contains(CloneFlags::CHILD_CLEARTID) {
        new_task.tid_address_mut().clear_child_tid = Some(chilren_tid_ptr);
    }
    if flags.contains(CloneFlags::SETTLS) {
        new_task.trap_context_mut().set_user_tp(tls_ptr);
    }

    log::info!("[sys_clone] who is your parent? {}", new_task.ppid());
    spawn_user_task(new_task);
    log::info!("[sys_clone] clone success",);

    task.set_is_yield(true);

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

    envs.push(String::from(
        r#"PATH=/:/bin:/sbin:/usr/bin:/usr/local/bin:/usr/local/sbin:"#,
    ));

    log::info!("[sys_execve] task: {:?}", task.get_name());
    log::info!("[sys_execve] args: {args:?}");
    log::info!("[sys_execve] envs: {envs:?}");
    log::info!("[sys_execve] path: {path:?}");

    let dentry = {
        let path = Path::new(sys_root_dentry(), path);
        let dentry = path.walk()?;
        if !dentry.is_negative() && dentry.inode().unwrap().inotype() == InodeType::SymLink {
            Path::resolve_symlink_through(Arc::clone(&dentry))?
        } else {
            dentry
        }
    };

    let file = <dyn File>::open(dentry)?;
    log::info!("[sys_execve]: open file");

    let mut name = String::new();
    args.iter().for_each(|arg| {
        name.push_str(arg);
        name.push(' ');
    });
    task.execve(file, args, envs, name)?;
    log::info!("[sys_execve]: finish execve and convert to a new task");
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
    log::error!("[sys_setgid] unimplemented call gid: {gid}");
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
    log::error!("[sys_setuid] unimplemented call uid: {uid}");
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
        TASK_MANAGER.get_task(pid).ok_or(SysError::ENOMEM)?
    } else {
        current_task()
    };

    let pgid = if pgid == 0 {
        task.process().get_pgid()
    } else {
        pgid
    };

    *task.pgid_mut().lock() = pgid;

    Ok(0)
}

/// `geteuid()` returns the effective user ID of the calling process.
pub fn sys_geteuid() -> SyscallResult {
    Ok(0)
}

/// `getegid()` returns the effective user ID of the calling process.
pub fn sys_getegid() -> SyscallResult {
    log::error!("[getegid] unimplemented call");
    Ok(0)
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

    let ptask = if pid == 0 {
        task.clone()
    } else {
        TASK_MANAGER.get_task(pid).ok_or(SysError::EINVAL)?
    };

    let resource = Resource::from_repr(resource).ok_or(SysError::EINVAL)?;

    log::debug!("[prlimit64] pid: {pid}, resource: {resource:?}");

    if !olimit.is_null() {
        let limit = match resource {
            Resource::STACK => {
                let rstack = RLimit::one(USER_STACK_SIZE, USER_STACK_SIZE);
                rstack
            }
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
