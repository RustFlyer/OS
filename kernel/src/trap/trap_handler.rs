use super::trap_env::set_kernel_stvec;
use crate::syscall;
use crate::task::{Task, TaskState, yield_now};
use crate::trap::load_trap_handler;
use crate::vm::mem_perm::MemPerm;
use alloc::sync::Arc;
use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{scause, sepc, sstatus::FS, stval},
};
use timer::TIMER_MANAGER;

/// handle exception or interrupt from a task, return if success
#[unsafe(no_mangle)]
pub async fn trap_handler(task: &Arc<Task>) -> bool {
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();

    log::trace!("[trap_handler] user task trap into kernel");
    log::trace!("[trap_handler] sepc:{:#x}, stval:{:#x}", sepc, stval);

    unsafe { load_trap_handler() };

    match cause {
        Trap::Exception(e) => {
            user_exception_handler(task, Exception::from_number(e).unwrap()).await
        }
        Trap::Interrupt(i) => {
            user_interrupt_handler(task, Interrupt::from_number(i).unwrap()).await
        }
    }
    true
}

pub async fn user_exception_handler(task: &Arc<Task>, e: Exception) {
    let mut cx = task.trap_context_mut();
    match e {
        // 系统调用
        Exception::UserEnvCall => {
            log::trace!("[trap_handler] user env call");
            let syscall_no = cx.syscall_no();
            cx.sepc_forward();

            let sys_ret = syscall(syscall_no, cx.syscall_args()).await;

            cx = task.trap_context_mut();
            cx.set_user_a0(sys_ret);
        }
        // 内存错误
        Exception::StorePageFault | Exception::InstructionPageFault | Exception::LoadPageFault => {
            let access = match e {
                Exception::InstructionPageFault => MemPerm::X,
                Exception::LoadPageFault => MemPerm::R,
                Exception::StorePageFault => MemPerm::W | MemPerm::R,
                _ => unreachable!(),
            };
            if let Err(e) = task
                .addr_space_mut()
                .lock()
                .handle_page_fault(VirtAddr::new(stval::read()), access)
            {
                // Should send a `SIGSEGV` signal to the task
                log::debug!(
                    "[user_exception_handler] unsolved page fault at {:#x}, access: {:?}, error: {:?}",
                    stval::read(),
                    access,
                    e
                );
                unimplemented!();
            }
        }
        // 非法指令
        Exception::IllegalInstruction => {
            log::warn!(
                "[trap_handler] detected illegal instruction, stval {:#x}, sepc {:#x}",
                stval::read(),
                sepc::read(),
            );
            task.set_state(TaskState::Zombie);
        }
        // 其他异常
        e => {
            log::warn!("Unknown user exception: {:?}", e);
        }
    }
}

pub async fn user_interrupt_handler(task: &Arc<Task>, i: Interrupt) {
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
                yield_now().await;
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
                "[trap_handler] Unsupported interrupt {:?}, stval = {:#x}, sepc = {:#x}",
                scause::read().cause(),
                stval::read(),
                sepc::read(),
            );
        }
    }
}
