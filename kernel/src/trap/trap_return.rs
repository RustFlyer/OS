use super::trap_context::TrapContext;
use crate::task::Task;
use crate::trap::trap_env;
use alloc::sync::Arc;

use arch::trap::disable_interrupt;

unsafe extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

/// Trap return to user mode.
pub fn trap_return(task: &Arc<Task>) {
    disable_interrupt();
    trap_env::set_user_trap_entry();

    // restore registers situations:
    // 1. current task yields after last trap.
    // 2. current task gets into sig-handler.
    let trap_cx = task.trap_context_mut();

    // assert that interrupt will be disabled when trap returns
    #[cfg(target_arch = "riscv64")]
    assert!(!(trap_cx.sstatus.sie()));

    task.timer_mut().switch_to_user();
    unsafe {
        let ptr = trap_cx as *mut TrapContext;
        __return_to_user(ptr);
    }
    task.timer_mut().switch_to_kernel();
}
