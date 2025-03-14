use super::trap_context::TrapContext;
use crate::task::{Task, TaskState};
use crate::trap::{self, csr_env};
use alloc::sync::Arc;
use arch::riscv64::{
    interrupt::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_nx_timer_irq},
};
use csr_env::{set_kernel_trap, set_user_trap};
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    sstatus::FS,
    stval,
};

unsafe extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

/// Trap return to user mode.
#[unsafe(no_mangle)]
pub fn trap_return(task: &Arc<Task>) {
    log::info!("[kernel] trap return to user...");
    unsafe {
        arch::riscv64::interrupt::disable_interrupt();
        csr_env::set_user_trap();
        // warn: stvec 不能在下面被改变。
        // 一个隐藏的错误是隐式使用 `UserPtr`，这将改变 stvec 为 `__trap_from_kernel`。
    };
    let mut timer = task.timer_mut();
    timer.record_trap_return();

    // 如果需要，恢复浮点寄存器。
    // 两种情况需要恢复寄存器：
    // 1. 这个任务在最后一次陷阱后已经让出了 CPU
    // 2. 这个任务遇到了信号处理程序
    task.trap_context_mut().restore_fx();
    task.trap_context_mut().sstatus.set_fs(FS::Clean);
    assert!(!(task.trap_context_mut().sstatus.sie()));
    assert!(!(task.is_in_state(TaskState::Zombie) || task.is_in_state(TaskState::Waiting)));
    unsafe {
        let ptr = task.trap_context_mut() as *mut TrapContext;
        __return_to_user(ptr);
        // note：当用户陷入内核时，下次会回到这里并返回到 `user_loop` 函数。
    }
    task.trap_context_mut()
        .mark_dirty(task.trap_context_mut().sstatus);
    timer = task.timer_mut();
    timer.record_trap();
}
