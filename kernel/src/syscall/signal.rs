use crate::{
    processor::current_task,
    task::{
        manager::TASK_MANAGER, sig_members::{Action, ActionType, SigAction, SigContext, SIG_DFL, SIG_IGN}, signal::{futex::{futex_manager, FutexAddr, FutexHashKey, FutexOp, FutexWaiter}, sig_info::*}, TaskState
    },
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use config::process::INIT_PROC_ID;
use mm::address::VirtAddr;
use osfuture::suspend_now;
use systype::{SysError, SyscallResult};
use time::{TimeSpec, TimeValue};
use mm::address::PhysAddr;

/// futex - fast user-space locking
/// # Arguments
/// - `uaddr`: points  to the futex word.  On all platforms, futexes are
///   four-byte integers that must be aligned on a four-byte boundary.
/// - `futex_op`: The operation to perform on the futex. The argument
///   consists of two parts: a command that specifies the operation to be
///   performed, bitwise ORed with zero or more options that modify the
///   behaviour of the operation.
/// - `val`: a value whose meaning and  purpose  depends on futex_op.
/// - `timeout`: a pointer to a timespec structure that specifies a timeout
///   for the operation.
/// - `uaddr2`: a pointer to a second futex word that is employed by the
///   operation.
/// - `val3`: depends on the operation.
pub async fn sys_futex(
    uaddr: FutexAddr,
    futex_op: i32,
    val: u32,
    timeout: usize,
    uaddr2: usize,
    val3: u32,
) -> SyscallResult {
    let mut futex_op = FutexOp::from_bits_truncate(futex_op);
    let task = current_task();
    uaddr.check()?;
    let is_private = futex_op.contains(FutexOp::Private);
    futex_op.remove(FutexOp::Private);
    let key = if is_private {
        FutexHashKey::Private {
            mm: task.raw_space_ptr(),
            vaddr: uaddr.addr,
        }
    } else {
        // to physical address
        let vaddr = VirtAddr::new(uaddr.raw());
        let ppn = task.addr_space().page_table.find_entry(vaddr.page_number())
            .ok_or(SysError::EFAULT)?
            .ppn();
        let paddr = PhysAddr::new(ppn.address().to_usize() + vaddr.page_offset());
        FutexHashKey::Shared { paddr }
    };
    log::info!(
        "[sys_futex] {:?} uaddr:{:#x} key:{:?}",
        futex_op,
        uaddr.raw(),
        key
    );

    match futex_op {
        FutexOp::Wait => {
            let res = uaddr.read();
            if res != val {
                log::info!(
                    "[futex_wait] value in {} addr is {res} but expect {val}",
                    uaddr.addr.to_usize()
                );
                return Err(SysError::EAGAIN);
            }
            futex_manager().add_waiter(
                &key,
                FutexWaiter {
                    tid: task.tid(),
                    waker: task.get_waker(),
                },
            );
            
            task.set_state(TaskState::Interruptable);

            let wake_up_signal = !*task.sig_mask_mut();
            task.set_wake_up_signal(wake_up_signal);
            if timeout == 0 {
                suspend_now().await;
            } else {
                let timeout = unsafe { UserReadPtr::<TimeSpec>::new(timeout as usize, &task.addr_space()).read() }?;
                log::info!("[futex_wait] waiting for {:?}", timeout);
                if !timeout.is_valid() {
                    return Err(SysError::EINVAL);
                }
                let rem = task.suspend_timeout(timeout.into()).await;
                if rem.is_zero() {
                    futex_manager().remove_waiter(&key, task.tid());
                }
            }
            if task.sig_manager_mut().has_expect_signals(wake_up_signal) {
                log::info!("[sys_futex] Woken by signal");
                futex_manager().remove_waiter(&key, task.tid());
                return Err(SysError::EINTR);
            }
            log::info!("[sys_futex] I was woken");
            task.set_state(TaskState::Running);
            Ok(0)
        }
        FutexOp::Wake => {
            let n_wake = futex_manager().wake(&key, val)?;
            return Ok(n_wake);
        }
        FutexOp::Requeue => {
            let n_wake = futex_manager().wake(&key, val)?;
            let new_key = if is_private {
                FutexHashKey::Private {
                    mm: task.raw_space_ptr(),
                    vaddr: VirtAddr::new(uaddr2),
                }
            } else {
                // to physical address
                let vaddr = VirtAddr::new(uaddr2);
                let ppn = task.addr_space().page_table.find_entry(vaddr.page_number())
                    .ok_or(SysError::EFAULT)?
                    .ppn();
                let paddr = PhysAddr::new(ppn.address().to_usize() + vaddr.page_offset());
                FutexHashKey::Shared { paddr }
            };
            futex_manager().requeue_waiters(key, new_key, timeout)?;
            Ok(n_wake)
        }
        FutexOp::CmpRequeue => {
            if uaddr.read() as u32 != val3 {
                return Err(SysError::EAGAIN);
            }
            let n_wake = futex_manager().wake(&key, val)?;
            let new_key = if is_private {
                FutexHashKey::Private {
                    mm: task.raw_space_ptr(),
                    vaddr: VirtAddr::new(uaddr2),
                }
            } else {
                // to physical address
                let vaddr = VirtAddr::new(uaddr2);
                let ppn = task.addr_space().page_table.find_entry(vaddr.page_number())
                    .ok_or(SysError::EFAULT)?
                    .ppn();
                let paddr = PhysAddr::new(ppn.address().to_usize() + vaddr.page_offset());
                FutexHashKey::Shared { paddr }
            };
            futex_manager().requeue_waiters(key, new_key, timeout)?;
            Ok(n_wake)
        }

        _ => panic!("unimplemented futexop {:?}", futex_op),
    }
}

/// - if pid > 0, send a SigInfo built on sig_code to the process with pid
/// - If pid = -1, then sig is sent to every process for which the calling
///   process has permission to send signals, except for process 1 (init)
/// to do: broadcast(to process group) when pid <= 0; permission check when sig_code == 0; i32 or u32
pub fn sys_kill(pid: isize, sig_code: i32) -> SyscallResult {
    // log::error!("[sys_kill] try to send sig_code {} to pid {}", sig_code, pid);
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
            log::error!("[sys_kill] Send {sig_code} to {pid}");
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
                Ok(
                    if task.pid() != INIT_PROC_ID && task.is_process() && sig.raw() != 0 {
                        task.receive_siginfo(SigInfo {
                            sig,
                            code: SigInfo::USER,
                            details: SigDetails::Kill { pid: task.pid() },
                        });
                    },
                )
            })?;
        }

        _ => {
            todo!()
        }
    }
    log::error!("[sys_kill] out");
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
            prev_sa.write(prev.into())?;
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
    log::trace!("[sys_rt_sigreturn] sig_cx_ptr: {sig_cx_ptr:?}");
    unsafe {
        let sig_cx = sig_cx_ptr.read()?;
        *mask = sig_cx.mask;
        // TODO: no sig_stack for now so don't need to restore
        trap_cx.sepc = sig_cx.user_reg[0];
        trap_cx.user_reg = sig_cx.user_reg;
    }
    // log::debug!("[sys_sigreturn] trap context: {:?}", trap_cx.user_reg);
    Ok(trap_cx.user_reg[10])
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

        // log::info!("[sys_rt_sigaction] new action: {:?}", action);

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
            let _ = prev_mask.write(*mask)?;
        }
    }

    if !input_mask.is_null() {
        unsafe {
            let input = input_mask.read()?;
            log::info!("[sys_rt_sigmask] input:{input:#x}");

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
                thread.receive_siginfo(SigInfo {
                    sig,
                    code: SigInfo::TKILL,
                    details: SigDetails::Kill { pid: task.pid() },
                });
                return Ok(0);
            }
        }
        return Err(SysError::ESRCH);
    })
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
        log::warn!("[sys_rt_sigtimedwait] {:?}", timeout);
        task.suspend_timeout(timeout.into()).await;
    } else {
        suspend_now().await;
    }

    task.set_state(TaskState::Running);
    let si = task.with_mut_sig_manager(|pending| pending.dequeue_expect(set));
    if let Some(si) = si {
        log::warn!("[sys_rt_sigtimedwait] I'm woken by {:?}", si);
        if !info.is_null() {
            unsafe {
                info.write(si)?;
            }
        }
        Ok(si.sig.raw())
    } else {
        log::warn!("[sys_rt_sigtimedwait] I'm woken by timeout");
        Err(SysError::EAGAIN)
    }
}
