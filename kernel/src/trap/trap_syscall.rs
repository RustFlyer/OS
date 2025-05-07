use osfuture::yield_now;
use systype::SysError;

use crate::{syscall::syscall, task::Task};

pub async fn async_syscall(task: &Task) -> bool {
    // if task.is_yield() {
    //     yield_now().await;
    //     task.set_is_yield(false);
    // }

    if !task.is_syscall() {
        return false;
    }
    task.set_is_syscall(false);

    let mut cx = task.trap_context_mut();
    let syscall_no = cx.syscall_no();
    cx.sepc_forward();
    cx.save_last_user_a0();
    let sys_ret = syscall(syscall_no, cx.syscall_args()).await;
    cx = task.trap_context_mut();
    cx.set_user_a0(sys_ret);
    if sys_ret == -(SysError::EINTR as isize) as usize {
        log::warn!("[async_syscall] EINTR, set interrupted to true");
        return true;
    }
    false
}
