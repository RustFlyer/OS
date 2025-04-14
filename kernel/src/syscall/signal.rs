use crate::{
    processor::current_task,
    task::{
        manager::TASK_MANAGER,
        sig_members::{Action, SigContext},
        signal::sig_info::*,
    },
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use systype::{SysError, SyscallResult};

/// if pid > 0, send a SigInfo built on sig_code to the process with pid
/// TODO: broadcast(to process group) when pid <= 0; permission check when sig_code == 0; i32 or u32
pub fn sys_kill(sig_code: i32, pid: isize) -> SyscallResult {
    let sig = Sig::from_i32(sig_code);
    if !sig.is_valid() {
        log::error!("invalid sig_code: {:}", sig_code);
        return Err(SysError::EINTR);
    }

    match pid {
        _ if pid > 0 => {
            log::info!("[sys_kill] Send {sig_code} to {pid}");
            if let Some(task) = TASK_MANAGER.get_task(pid as usize) {
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
                return Err(SysError::ESRCH);
            }
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
pub fn sys_sigaction(
    sig_code: i32,
    mut new_sa: UserReadPtr<Action>,
    mut prev_sa: UserWritePtr<Action>,
) -> SyscallResult {
    let tid = current_task().tid();
    // maybe use TASK_MANAGER.get_task(tid).unwrap() to get handlers?
    let task = current_task();
    let mut handlers = task.sig_handlers_mut().lock();
    let sig = Sig::from_i32(sig_code);
    if !sig.is_valid() || matches!(sig, Sig::SIGKILL | Sig::SIGSTOP) {
        return Err(SysError::EINVAL);
    }
    log::info!(
        "[sys_sigaction] for {sig_code:?} signal in task {tid}, new handler:{new_sa:?}, save previous handler in:{prev_sa:?}"
    );

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
pub fn sys_sigmask(
    mode: usize,
    mut input_mask: UserReadPtr<SigSet>,
    mut prev_mask: UserWritePtr<SigSet>,
) -> SyscallResult {
    // Question: is it safe?
    let task = current_task();
    let mask = task.sig_mask_mut();
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
    let mut addr_space = task.addr_space_mut().lock().await;
    let mut sig_cx_ptr = UserReadPtr::<SigContext>::new(sig_cx_ptr, &mut *addr_space);
    log::trace!("[sys_rt_sigreturn] sig_cx_ptr: {sig_cx_ptr:?}");
    unsafe {
        let sig_cx = sig_cx_ptr.read()?;
        *mask = sig_cx.mask;
        // TODO: no sig_stack for now so don't need to restore
        trap_cx.sepc = sig_cx.user_reg[0];
        trap_cx.user_reg = sig_cx.user_reg;
    }
    Ok(trap_cx.user_reg[10])
}
