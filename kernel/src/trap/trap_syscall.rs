use osfuture::yield_now;

use crate::{syscall::syscall, task::Task};

pub async fn async_syscall(task: &Task) {
    if task.is_yield() {
        yield_now().await;
    }

    if !task.is_syscall() {
        return;
    }
    task.set_is_syscall(false);

    let mut cx = task.trap_context_mut();
    let syscall_no = cx.syscall_no();
    cx.sepc_forward();
    let sys_ret = syscall(syscall_no, cx.syscall_args()).await;
    cx = task.trap_context_mut();
    cx.set_user_ret_val(sys_ret);
}
