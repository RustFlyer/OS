use super::trap_context::TrapContext;
use crate::task::{Task, TaskState};
use crate::trap::trap_env;
use alloc::sync::Arc;

use arch::trap::disable_interrupt;

unsafe extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

/// Trap return to user mode.
#[unsafe(no_mangle)]
pub fn trap_return(task: &Arc<Task>) {
    disable_interrupt();
    trap_env::set_user_trap_entry();

    // restore registers situations:
    // 1. current task yields after last trap.
    // 2. current task gets into sig-handler.
    let trap_cx = task.trap_context_mut();
    // trap_cx.sstatus.set_fs(FS::Clean);

    // assert that interrupt will be disabled when trap returns
    assert!(!(trap_cx.sstatus.sie()));
    assert!(!(task.is_in_state(TaskState::Zombie) || task.is_in_state(TaskState::Sleeping)));

    task.timer_mut().switch_to_user();
    // log::info!("[trap_return] go to user space");
    unsafe {
        let ptr = trap_cx as *mut TrapContext;
        __return_to_user(ptr);
    }
    // log::info!("[trap_return] return from user space");
    task.timer_mut().switch_to_kernel();
}
