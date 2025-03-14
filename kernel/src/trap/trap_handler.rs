use super::csr_env::set_kernel_trap;
use crate::task::{Task, TaskState};
use alloc::sync::Arc;
use arch::riscv64::{
    interrupt::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_nx_timer_irq},
};
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    sstatus::FS,
    stval,
};
use timer::TIMER_MANAGER;

/// 处理用户空间的中断、异常或系统调用
/// 返回是否是系统调用并且被中断
#[unsafe(no_mangle)]
pub async fn trap_handler(task: &Arc<Task>) -> bool {
    unsafe { set_kernel_trap() };

    let cx = task.trap_context_mut();

    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();

    log::trace!("[trap_handler] user task trap into kernel");
    log::trace!("[trap_handler] sepc:{:#x}, stval:{:#x}", sepc, stval);
    unsafe { enable_interrupt() };

    match cause {
        Trap::Exception(e) => user_exception_handler(task, e),
        Trap::Interrupt(i) => user_interrupt_handler(task, i),
    }
    true
}

pub fn user_exception_handler(task: &Arc<Task>, e: Exception) {
    match e {
        // 系统调用
        Exception::UserEnvCall => {
            log::trace!("[trap_handler] user env call");
        }
        // 内存错误
        Exception::StorePageFault | Exception::InstructionPageFault | Exception::LoadPageFault => {
            todo!()
        }
        // 非法指令
        Exception::IllegalInstruction => {
            log::warn!(
                "[trap_handler] detected illegal instruction, stval {:#x}, sepc {:#x}",
                stval::read(),
                sepc::read(),
            );
            task.set_state(TaskState::Die);
        }
        // 其他异常
        e => {
            log::warn!("Unknown user exception: {:?}", e);
        }
    }
}

pub fn user_interrupt_handler(task: &Arc<Task>, i: Interrupt) {
    match i {
        // 时钟中断
        Interrupt::SupervisorTimer => {
            // note: 用户若频繁陷入内核，则可能是因为时钟中断未触发，
            // 而是 supervisor 模式下触发了，导致用户程序在 CPU 上运行了很长时间。
            log::trace!("[trap_handler] timer interrupt, sepc {:#x}", sepc::read());
            let current = get_time_duration();
            TIMER_MANAGER.check(current);
            unsafe { set_nx_timer_irq() };
            if executor::has_waiting_task() {
                // yield_now().await;
                todo!()
            }
        }
        // 外部中断
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
            // driver::get_device_manager_mut().handle_irq();
        }
        // 其他中断
        _ => {
            panic!(
                "[trap_handler] Unsupported trap {:?}, stval = {:#x}, sepc = {:#x}",
                scause::read().cause(),
                stval::read(),
                sepc::read(),
            );
        }
    }
}
