use crate::{
    task::{
        TASK_MANAGER,
        sig_members::{Action, SigActionFlag},
        signal::sig_info::*,
    },
    vm::user_ptr::UserReadPtr,
};
use alloc::task;
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

pub fn sys_sigaction(sig_code: i32, new_sa: UserReadPtr<Action>) {}

pub fn sys_sigmask() {}

pub fn sys_sigreturn() -> SyscallResult {
    Ok(())
}
