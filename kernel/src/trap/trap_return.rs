use super::trap_context::TrapContext;
use crate::task::{Task, TaskState};
use crate::trap::{self, trap_env};
use alloc::sync::Arc;
use arch::riscv64::{
    interrupt::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_nx_timer_irq},
};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{scause, sepc, sstatus::FS, stval},
};
use trap_env::{set_kernel_stvec, set_user_stvec};

unsafe extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

/// Trap return to user mode.
#[unsafe(no_mangle)]
pub fn trap_return(task: &Arc<Task>) {
    log::info!("[kernel] trap return to user...");
    unsafe {
        arch::riscv64::interrupt::disable_interrupt();
        trap_env::set_user_stvec();
        // warn: stvec 不能在下面被改变。
        // 一个隐藏的错误是隐式使用 `UserPtr`，这将改变 stvec 为 `__trap_from_kernel`。
    };
    let mut timer = task.timer_mut();
    timer.record_trap_return();

    // 两种情况需要恢复寄存器：
    // 1. 这个任务在最后一次陷阱后已经让出了 CPU
    // 2. 这个任务遇到了信号处理程序
    let mut trap_context_lock_mut = task.trap_context_spinlock_mut().lock();
    trap_context_lock_mut.restore_fx();
    trap_context_lock_mut.sstatus.set_fs(FS::Clean);
    assert!(!(trap_context_lock_mut.sstatus.sie()));
    assert!(!(task.is_in_state(TaskState::Zombie) || task.is_in_state(TaskState::Waiting)));
    unsafe {
        // let ptr = trap_context_mut as *mut TrapContext;
        let ptr = (&mut *trap_context_lock_mut) as *mut TrapContext;
        __return_to_user(ptr);
        // note：当用户陷入内核时，下次会回到这里并返回到 `user_loop` 函数。
        // 陷入内核后，不会有其他线程对trapcontext进行访问，该锁实际一直持有在本线程
        // 因此无需drop
    }
    trap_context_lock_mut = task.trap_context_spinlock_mut().lock();
    let new_sstatus = trap_context_lock_mut.sstatus;
    trap_context_lock_mut.mark_dirty(new_sstatus);
    timer = task.timer_mut();
    timer.record_trap();
}
