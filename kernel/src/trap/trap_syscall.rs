use systype::error::SysError;

use crate::{syscall::syscall, task::Task};

pub async fn async_syscall(task: &Task) -> bool {
    if !task.is_syscall() {
        return false;
    }
    task.set_is_syscall(false);

    let mut cx = task.trap_context_mut();
    let syscall_no = cx.syscall_no();
    cx.sepc_forward();
    cx.save_last_user_ret_val();
    let sys_ret = syscall(syscall_no, cx.syscall_args()).await;
    cx = task.trap_context_mut();
    cx.set_user_ret_val(sys_ret);
    if sys_ret == -(SysError::EINTR as isize) as usize {
        log::warn!("[async_syscall] EINTR, set interrupted to true");
        return true;
    }
    false
}
