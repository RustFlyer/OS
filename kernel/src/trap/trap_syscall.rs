use osfuture::yield_now;
use systype::SysError;

use crate::{processor::current_hart, syscall::syscall, task::Task};

pub async fn async_syscall(task: &Task) -> bool {
    if task.timer_mut().schedule_time_out() && executor::has_waiting_task_alone(current_hart().id) {
        // log::trace!("[trap_handler] {} yield", task.get_name());
        task.set_is_yield(true);
    }

    if task.is_yield() {
        yield_now().await;
    }

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
