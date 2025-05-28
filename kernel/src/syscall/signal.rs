use crate::{
    processor::current_task,
    task::{
        TaskState,
        futex::{
            FutexAddr, FutexHashKey, FutexOp, FutexWaiter, futex_manager, single_futex_manager,
        },
        manager::TASK_MANAGER,
        sig_members::{Action, ActionType, SIG_DFL, SIG_IGN, SigAction, SigContext},
        signal::sig_info::*,
    },
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use alloc::string::String;
use config::process::INIT_PROC_ID;
use osfuture::suspend_now;
use systype::{SysError, SyscallResult};
use time::{TimeSpec, TimeValue};

/// futex - fast user-space locking
///
/// The futex() system call provides a method for waiting until a certain condition becomes true.
/// It is typically used as a blocking construct in the context of shared-memory synchronization.
/// When using futexes, the majority of the synchronization operations are performed in user space.
/// A user-space program employs the futex() system call only when it is likely that the program
/// has to block for a longer time until the condition becomes true. Other futex() operations can
/// be used to wake any processes or threads waiting for a particular condition.
///
/// A futex is a 32-bit value—referred to below as a futex word—whose address is supplied to the
/// futex() system call. (Futexes are 32 bits in size on all platforms, including 64-bit systems.)
/// All futex operations are governed by this value. In order to share a futex between processes,
/// the futex is placed in a region of shared memory, created using (for example) mmap(2) or shmat(2).
/// (Thus, the futex word may have different virtual addresses in different processes, but these
/// addresses all refer to the same location in physical memory.) In a multithreaded program, it
/// is sufficient to place the futex word in a global variable shared by all threads.
///
/// # Arguments
/// - `uaddr`: points  to the futex word.  On all platforms, futexes are four-byte integers that
///   must be aligned on a four-byte boundary.
/// - `futex_op`: The operation to perform on the futex. The argument consists of two parts: a
///   command that specifies the operation to be performed, bitwise ORed with zero or more options
///   that modify the behaviour of the operation.
/// - `val`: a value whose meaning and purpose depends on `futex_op`.
/// - `timeout`: a pointer to a timespec structure that specifies a timeout for the operation.
///   However, notwithstanding the prototype shown above, for some operations, the least significant
///   four bytes of this argument are instead used as an integer whose meaning is determined by the
///   operation. For these operations, the kernel casts the timeout value first to unsigned long,
///   then to uint32_t, and in the remainder of this page, this argument is referred to as val2 when
///   interpreted in this fashion.
/// - `uaddr2`: a pointer to a second futex word that is employed by the operation.
/// - `val3`: depends on the operation.
///
/// # Op
///
/// - `FUTEX_WAIT_BITSET` (since Linux 2.6.25)
///   This operation is like `FUTEX_WAIT` except that `val3` is used to provide a 32-bit bit mask to
///   the kernel. This bit mask, in which at least one bit must be set, is stored in the kernel-internal
///   state of the waiter. See the description of `FUTEX_WAKE_BITSET` for further details.
///   If `timeout` is not NULL, the structure it points to specifies an absolute `timeout` for the wait
///   operation. If `timeout` is NULL, the operation can block indefinitely. The `uaddr2` argument is ignored.
///
/// - `FUTEX_WAKE_BITSET` (since Linux 2.6.25)
///   This operation is the same as `FUTEX_WAKE` except that the `val3` argument is used to provide a 32-bit
///   bit mask to the kernel. This bit mask, in which at least one bit must be set, is used to select which
///   waiters should be woken up. The selection is done by a bitwise AND of the "wake" bit mask (i.e.,
///   the value in `val3`) and the bit mask which is stored in the kernel-internal state of the waiter (the
///   "wait" bit mask that is set using `FUTEX_WAIT_BITSET`). All of the waiters for which the result of the
///   AND is nonzero are woken up; the remaining waiters are left sleeping. The effect of `FUTEX_WAIT_BITSET`
///   and `FUTEX_WAKE_BITSET` is to allow selective wake-ups among multiple waiters that are blocked on
///   the same futex. However, note that, depending on the use case, employing this bit-mask multiplexing
///   feature on a futex can be less efficient than simply using multiple futexes, because employing bit-mask
///   multiplexing requires the kernel to check all waiters on a futex, including those that are not interested
///   in being woken up (i.e., they do not have the relevant bit set in their "wait" bit mask).
///   The constant `FUTEX_BITSET_MATCH_ANY`, which corresponds to all 32 bits set in the bit mask, can be used as
///   the `val3` argument for `FUTEX_WAIT_BITSET` and `FUTEX_WAKE_BITSET`. Other than differences in the handling of
///   the `timeout` argument, the FUTEX_WAIT operation is equivalent to FUTEX_WAIT_BITSET with `val3` specified as
///   `FUTEX_BITSET_MATCH_ANY`; that is, allow a wake-up by any waker. The `FUTEX_WAKE` operation is equivalent to
///   `FUTEX_WAKE_BITSET` with `val3` specified as `FUTEX_BITSET_MATCH_ANY`; that is, wake up any waiter(s).
///   The `uaddr2` and `timeout` arguments are ignored.
pub async fn sys_futex(
    uaddr: usize,
    futex_op: i32,
    val: u32,
    timeout: usize,
    uaddr2: usize,
    val3: u32,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let allop = FutexOp::exstract_futex_flags(futex_op);
    let futex_addr = FutexAddr::new_with_check(uaddr, &addrspace)?;
    let is_private = allop.contains(FutexOp::Private);
    // For Debug
    // let is_multi_group = false;
    // let val3 = 0xffffffff;
    let is_multi_group = allop.contains(FutexOp::WaitBitset) | allop.contains(FutexOp::WaitBitset);

    let key = FutexHashKey::new_key(futex_addr.addr(), addrspace.clone(), is_private)?;

    let faddr = futex_addr.addr();
    log::info!(
        "[sys_futex] {} uaddr:{:#x} key:{:?}, op: {:#x} {:?} val: {:#X}",
        task.get_name(),
        faddr,
        key,
        futex_op,
        allop,
        val
    );

    // match op.intersection(FutexOp::MAINOPMASK) {
    let op = FutexOp::exstract_main_futex_flags(futex_op);
    match op {
        FutexOp::WaitBitset | FutexOp::Wait => {
            log::debug!("[sys_futex] Wait Get Locked op: {:?} mask: {:#x}", op, val3);
            let r = futex_addr.read();
            if r != val {
                log::debug!("[sys_futex] r: {:#x} val: {:#x}", r, val);
                return Err(SysError::EAGAIN);
            }

            let new_waker = FutexWaiter::new(&task);
            futex_manager(is_multi_group, val3).add_waiter(&key, new_waker)?;

            task.set_state(TaskState::Interruptable);
            let wake_up_signal = !*task.sig_mask_mut();
            task.set_wake_up_signal(wake_up_signal);
            if timeout != 0 {
                let ts = unsafe { UserReadPtr::<TimeSpec>::new(timeout, &addrspace).read() }?;
                (!ts.is_valid()).then_some(SysError::EINVAL);
                log::debug!("[sys_futex] Wait for {:?}", ts);

                let rem = task.suspend_timeout(ts.into()).await;
                let tid = task.tid();
                rem.is_zero()
                    .then(|| futex_manager(is_multi_group, val3).rm_waiter(&key, tid));
            } else {
                log::warn!("[sys_futex] task {} has been suspended", task.get_name());
                suspend_now().await;
                log::warn!("[sys_futex] task {} has been woken", task.get_name());
            }

            if task.sig_manager_mut().has_expect_signals(wake_up_signal) {
                let _ = futex_manager(is_multi_group, val3).rm_waiter(&key, task.tid());
                return Err(SysError::EINTR);
            }

            task.set_state(TaskState::Running);
            Ok(0)
        }
        FutexOp::WakeBitset | FutexOp::Wake => {
            log::debug!("[sys_futex] Wake");
            let n_wake = futex_manager(is_multi_group, val3).wake(&key, val)?;
            Ok(n_wake)
        }
        FutexOp::Requeue | FutexOp::CmpRequeue => {
            if op.contains(FutexOp::CmpRequeue) && futex_addr.read() as u32 != val3 {
                return Err(SysError::EAGAIN);
            }

            let n_wake = single_futex_manager().wake(&key, val)?;
            let new_key = FutexHashKey::new_key(uaddr2, addrspace.clone(), is_private)?;
            single_futex_manager().requeue_waiters(key, new_key, timeout)?;
            Ok(n_wake)
        }

        _ => {
            log::error!(
                "[panic?] unimplemented futexop({:#x}:{}) {:?} called by {}",
                futex_op,
                futex_op as usize,
                op,
                task.get_name()
            );
            Err(SysError::EINVAL)
        }
    }
}

/// - if pid > 0, send a SigInfo built on sig_code to the process with pid
/// - If pid = -1, then sig is sent to every process for which the calling
///   process has permission to send signals, except for process 1 (init)
///
/// TODO: broadcast(to process group) when pid <= 0; permission check when sig_code == 0; i32 or u32
pub fn sys_kill(pid: isize, sig_code: i32) -> SyscallResult {
    log::debug!(
        "[sys_kill] try to send sig_code {} to pid {}",
        sig_code,
        pid
    );
    // TASK_MANAGER.for_each(|task| {
    //     log::error!("[sys_kill] existing task's pid: {}", task.tid());
    //     Ok(())
    // })?;
    let sig = Sig::from_i32(sig_code);
    if !sig.is_valid() {
        log::error!("invalid sig_code: {:}", sig_code);
        return Err(SysError::EINTR);
    }

    match pid {
        _ if pid > 0 => {
            log::info!("[sys_kill] Send {sig_code} to {pid}");
            if let Some(task) = TASK_MANAGER.get_task(pid as usize) {
                log::debug!(
                    "[sys_kill] thread {} name {} gets killed",
                    task.tid(),
                    task.get_name()
                );
                if !task.is_process() {
                    return Err(SysError::ESRCH);
                } else {
                    task.receive_siginfo(SigInfo {
                        sig,
                        code: SigInfo::USER,
                        details: SigDetails::Kill { pid: task.pid() },
                    });
                }
            } else {
                log::error!("[sys_kill] can't find assigned pid.");
                return Ok(0);
            }
        }

        -1 => {
            TASK_MANAGER.for_each(|task| {
                if task.pid() != INIT_PROC_ID && task.is_process() && sig.raw() != 0 {
                    task.receive_siginfo(SigInfo {
                        sig,
                        code: SigInfo::USER,
                        details: SigDetails::Kill { pid: task.pid() },
                    });
                }
                Ok(())
            })?;
        }

        _ => {
            todo!()
        }
    }
    Ok(0)
}

/// set a new action(including ignore) for current task and save the previous
/// user should offer two new Action instances, one of them(prev_sa) can be default
/// if no need for restore, prev_sa can be NULL
/// Question: Current task could be a thread, so it's process doesn't change. Does this work?
pub fn sys_sigaction(sig_code: i32, new_sa: usize, prev_sa: usize) -> SyscallResult {
    // maybe use TASK_MANAGER.get_task(tid).unwrap() to get handlers?
    let task = current_task();
    let tid = task.tid();
    let addrspace = task.addr_space();

    let mut new_sa = UserReadPtr::<Action>::new(new_sa, &addrspace);
    let mut prev_sa = UserWritePtr::<Action>::new(prev_sa, &addrspace);

    let mut handlers = task.sig_handlers_mut().lock();
    let sig = Sig::from_i32(sig_code);

    log::info!(
        "[sys_sigaction] for {sig_code:?} signal in task {tid}, new handler:{new_sa:?}, save previous handler in:{prev_sa:?}"
    );

    if !sig.is_valid() || matches!(sig, Sig::SIGKILL | Sig::SIGSTOP) {
        return Err(SysError::EINVAL);
    }

    if !prev_sa.is_null() {
        let prev = handlers.get(sig);
        unsafe {
            prev_sa.write(prev)?;
        }
    }

    if !new_sa.is_null() {
        unsafe {
            let mut sa = new_sa.read()?;
            sa.mask.remove_signal(Sig::SIGKILL);
            sa.mask.remove_signal(Sig::SIGSTOP);

            log::info!("[sys_sigaction] new Action:{:?}", sa);
            handlers.update(sig, sa);
        }
    }

    Ok(0)
}

/// set, add or remove signals from task's mask and save the previous
/// user should offer two SigSet instance, one of them(prev_mask) can be default
/// if no need for restore, prev_mask can be NULL
/// only affects current thread
pub fn sys_sigmask(mode: usize, input_mask: usize, prev_mask: usize) -> SyscallResult {
    // Question: is it safe?
    let task = current_task();
    let mask = task.sig_mask_mut();
    let addrspace = task.addr_space();

    let mut input_mask = UserReadPtr::<SigSet>::new(input_mask, &addrspace);
    let mut prev_mask = UserWritePtr::<SigSet>::new(prev_mask, &addrspace);

    // define modes
    const SIGBLOCK: usize = 0;
    const SIGUNBLOCK: usize = 1;
    const SIGSETMASK: usize = 2;

    if !prev_mask.is_null() {
        unsafe {
            prev_mask.write(*mask)?;
        }
    }

    if !input_mask.is_null() {
        unsafe {
            let input = input_mask.read()?;
            log::info!("[sys_sigmask] input:{input:#x}");

            match mode {
                SIGBLOCK => {
                    *mask |= input;
                }
                SIGUNBLOCK => {
                    mask.remove(input);
                }
                SIGSETMASK => {
                    *mask = input;
                }
                _ => {
                    return Err(SysError::EINVAL);
                }
            }
            //Question: Why mask can't be derefereced but can be Deref automatically to call method?
            mask.remove_signal(Sig::SIGKILL);
            mask.remove_signal(Sig::SIGCONT);
        }
    }
    Ok(0)
}

pub async fn sys_sigreturn() -> SyscallResult {
    let task = current_task();
    let trap_cx = task.trap_context_mut();
    let mask = task.sig_mask_mut();
    let sig_cx_ptr = task.get_sig_cx_ptr();
    let addr_space = task.addr_space();
    let mut sig_cx_ptr = UserReadPtr::<SigContext>::new(sig_cx_ptr, &addr_space);
    // log::debug!("[sys_sigreturn] sig_cx_ptr: {sig_cx_ptr:?}");
    // restore trap context before sig handle

    let mut rs = String::new();
    trap_cx
        .user_reg
        .iter()
        .enumerate()
        .for_each(|(idx, u)| rs.push_str(format!("r[{idx:02}] = {u:#x}, ").as_str()));
    log::debug!(
        "[sys_sigreturn] task: {} ,before trap context: [{:?}]",
        task.get_name(),
        rs
    );
    unsafe {
        let sig_cx = sig_cx_ptr.read()?;
        *mask = sig_cx.mask;
        // TODO: no sig_stack for now so don't need to restore
        trap_cx.sepc = sig_cx.user_reg[0];
        //log::debug!("[sys_sigreturn] restore trap_cx a0: {} with backup in sig_cx: {}", trap_cx.user_reg[10], sig_cx.user_reg[10]);
        trap_cx.user_reg = sig_cx.user_reg;
    }
    let mut rs = String::new();
    trap_cx
        .user_reg
        .iter()
        .enumerate()
        .for_each(|(idx, u)| rs.push_str(format!("r[{idx:02}] = {u:#x}, ").as_str()));
    log::debug!(
        "[sys_sigreturn] task: {} ,after trap context: [{:?}]",
        task.get_name(),
        rs
    );
    // log::debug!("sig: {:#x}", task.sig_manager_mut().bitmap.bits());
    // its return value is the a0 before signal interrupt, so that it won't be changed in async_syscall
    // trap_cx.display();
    Ok(trap_cx.get_user_a0())
}

/// The original Linux system call was named sigaction(). However, with the addition
/// of real-time signals in Linux 2.2, the fixed-size, 32-bit sigset_t type supported
/// by that system call was no longer fit for purpose.
///
/// Consequently, a new system call, `rt_sigaction()`, was added to support an enlarged `sigset_t` type.
/// The new system call takes a fourth argument, size_t `sigsetsize`, which specifies the size in bytes of
/// the signal sets in `act.sa_mask` and `oldact.sa_mask`. This argument is currently
/// required to have the value sizeof(sigset_t) (or the error EINVAL results).
///
/// The glibc sigaction() wrapper function hides these details from us, transparently
/// calling rt_sigaction() when the kernel provides it.
#[allow(non_snake_case)]
pub fn sys_rt_sigaction(
    signum: i32,
    new_sa: usize,
    prev_sa: usize,
    sigsetsize: usize,
) -> SyscallResult {
    log::info!(
        "[sys_rt_sigaction] signum: {signum:?}, new_sa: {new_sa:#x}, prev_sa: {prev_sa:#x}, sigsetsize: {sigsetsize:?}"
    );

    let task = current_task();
    let addrspace = task.addr_space();
    let signum = Sig::from_i32(signum);

    if !signum.is_valid() || matches!(signum, Sig::SIGKILL | Sig::SIGSTOP) {
        return Err(SysError::EINVAL);
    }

    let mut old_sa = UserWritePtr::<SigAction>::new(prev_sa, &addrspace);
    let mut new_sa = UserReadPtr::<SigAction>::new(new_sa, &addrspace);

    if !old_sa.is_null() {
        let old = task.sig_handlers_mut().lock().get(signum);
        unsafe {
            old_sa.write(old.into())?;
        }
    }

    if !new_sa.is_null() {
        let mut action = unsafe { new_sa.read()? };

        log::info!("[sys_rt_sigaction] new action: {:?}", action);
        log::info!(
            "[sys_rt_sigaction] new action restorer: {:#x}",
            action.restorer
        );

        action.sa_mask.remove(SigSet::SIGKILL | SigSet::SIGSTOP);

        let atype = match action.sa_handler {
            SIG_DFL => ActionType::default(signum),
            SIG_IGN => ActionType::Ignore,
            entry => {
                log::info!(
                    "[sys_rt_sigaction] task [{}] set code entry: {:#x}",
                    task.get_name(),
                    entry
                );
                ActionType::User { entry }
            }
        };

        let new = Action {
            atype,
            flags: action.sa_flags,
            mask: action.sa_mask,
        };

        // log::info!("[sys_rt_sigaction] new:{:?}", new);
        task.sig_handlers_mut().lock().update(signum, new);
    }
    Ok(0)
}

/// The original Linux system call was named `sigprocmask()`. However, with
/// the addition of real-time signals in Linux 2.2, the fixed-size, 32-bit `sigset_t`
/// (referred to as old_kernel_sigset_t in this manual page) type supported by that
/// system call was no longer fit for purpose.
///
/// Consequently, a new system call, `rt_sigprocmask()`, was added to support an enlarged
/// `sigset_t` type (referred to as kernel_sigset_t in this manual page).
///
/// The new system call takes a fourth argument, size_t `sigsetsize`, which specifies
/// the size in bytes of the signal sets in `set` and `oldset`. This argument is currently
/// required to have a fixed architecture specific value (equal to sizeof(kernel_sigset_t)).
pub fn sys_rt_sigmask(
    how: usize,
    input_mask: usize,
    prev_mask: usize,
    sigsetsize: usize,
) -> SyscallResult {
    let task = current_task();
    let mask = task.sig_mask_mut();
    let addrspace = task.addr_space();

    assert!(sigsetsize == 8);

    let mut input_mask = UserReadPtr::<SigSet>::new(input_mask, &addrspace);
    let mut prev_mask = UserWritePtr::<SigSet>::new(prev_mask, &addrspace);

    // define modes
    const SIGBLOCK: usize = 0;
    const SIGUNBLOCK: usize = 1;
    const SIGSETMASK: usize = 2;

    if !prev_mask.is_null() {
        unsafe {
            prev_mask.write(*mask)?;
        }
    }

    if !input_mask.is_null() {
        unsafe {
            let input = input_mask.read()?;
            log::debug!("[sys_rt_sigmask] task {} input:{input:#x}", task.get_name());
            // log::warn!("[sys_rt_sigmask] how: {how:#x}");

            match how {
                SIGBLOCK => {
                    *mask |= input;
                }
                SIGUNBLOCK => {
                    mask.remove(input);
                }
                SIGSETMASK => {
                    *mask = input;
                }
                _ => {
                    return Err(SysError::EINVAL);
                }
            }
            //Question: Why mask can't be derefereced but can be Deref automatically to call method?
            mask.remove_signal(Sig::SIGKILL);
            mask.remove_signal(Sig::SIGCONT);
        }
    }
    Ok(0)
}

/// `tgkill()` sends the signal `sig` to the thread with the thread ID `tid` in the thread group `tgid`.
/// (By contrast, kill(2) can be used to send a signal only to a process (i.e., thread group)
/// as a whole, and the signal will be delivered to an arbitrary thread within that process.)
pub fn sys_tgkill(tgid: isize, tid: isize, signum: i32) -> SyscallResult {
    log::debug!("[sys_tgkill] tgid: {tgid}, tid: {tid}, signum: {signum}");
    let sig = Sig::from_i32(signum);
    if !sig.is_valid() || tgid < 0 || tid < 0 {
        return Err(SysError::EINVAL);
    }
    let task = TASK_MANAGER
        .get_task(tgid as usize)
        .ok_or(SysError::ESRCH)?;

    if !task.is_process() {
        return Err(SysError::ESRCH);
    }
    task.with_thread_group(|tg| -> SyscallResult {
        for thread in tg.iter() {
            if thread.tid() == tid as usize {
                log::debug!("thread [{}] recv sig {:?}", thread.get_name(), sig);
                thread.receive_siginfo(SigInfo {
                    sig,
                    code: SigInfo::TKILL,
                    details: SigDetails::Kill { pid: task.pid() },
                });
                return Ok(0);
            }
        }
        Err(SysError::ESRCH)
    })
}

/// tkill() is an obsolete predecessor to tgkill(). It allows
/// only the target thread ID to be specified, which may result in
/// the wrong thread being signaled if a thread terminates and its
/// thread ID is recycled. Avoid using this system call.
pub fn sys_tkill(tid: isize, sig: i32) -> SyscallResult {
    log::debug!("[sys_tkill] to tid: {tid}, signum: {sig}");
    let sig = Sig::from_i32(sig);
    if !sig.is_valid() || tid < 0 {
        return Err(SysError::EINVAL);
    }

    let task = TASK_MANAGER.get_task(tid as usize).ok_or(SysError::ESRCH)?;
    task.receive_siginfo(SigInfo {
        sig,
        code: SigInfo::TKILL,
        details: SigDetails::None,
    });
    Ok(0)
}

/// Suspends execution of the calling thread until one of the signals in set
/// is pending (If one of the signals in set is already pending for the
/// calling thread, sigwaitinfo() will return immediately.). It removes the
/// signal from the set of pending signals and returns the signal number
/// as its function result.
pub async fn sys_rt_sigtimedwait(set: usize, info: usize, timeout: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let mut set = UserReadPtr::<SigSet>::new(set, &addrspace);
    let mut info = UserWritePtr::<SigInfo>::new(info, &addrspace);
    let mut timeout = UserReadPtr::<TimeSpec>::new(timeout, &addrspace);

    let mut set = unsafe { set.read()? };
    set.remove(SigSet::SIGKILL | SigSet::SIGSTOP);
    let sig = task.with_mut_sig_manager(|pending| {
        if let Some(si) = pending.get_expect(set) {
            Some(si.sig)
        } else {
            pending.should_wake = set | SigSet::SIGKILL | SigSet::SIGSTOP;
            None
        }
    });

    if let Some(sig) = sig {
        return Ok(sig.raw());
    }

    task.set_state(TaskState::Interruptable);
    if !timeout.is_null() {
        let timeout = unsafe { timeout.read()? };
        if !timeout.is_valid() {
            return Err(SysError::EINVAL);
        }
        log::info!("[sys_rt_sigtimedwait] {:?}", timeout);
        task.suspend_timeout(timeout.into()).await;
    } else {
        suspend_now().await;
    }

    task.set_state(TaskState::Running);
    let si = task.with_mut_sig_manager(|pending| pending.dequeue_expect(set));
    if let Some(si) = si {
        log::info!("[sys_rt_sigtimedwait] I'm woken by {:?}", si);
        if !info.is_null() {
            unsafe {
                info.write(si)?;
            }
        }
        Ok(si.sig.raw())
    } else {
        log::info!("[sys_rt_sigtimedwait] I'm woken by timeout");
        Err(SysError::EAGAIN)
    }
}
