use mm::address::VirtAddr;
use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{scause, sepc, stval},
};

use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use timer::TIMER_MANAGER;

use crate::processor::current_task;
use crate::task::signal::sig_info::{Sig, SigDetails, SigInfo};
use crate::vm::mem_perm::MemPerm;

/// Kernel trap handler
#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(e) => kernel_exception_handler(Exception::from_number(e).unwrap(), stval),
        Trap::Interrupt(i) => kernel_interrupt_handler(Interrupt::from_number(i).unwrap(), stval),
    }
}

pub fn kernel_exception_handler(e: Exception, stval: usize) {
    let sepc = sepc::read();
    let scause = scause::read();
    let cause = scause.cause();
    match e {
        Exception::StorePageFault
        | Exception::InstructionPageFault
        | Exception::LoadPageFault => {
            log::info!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
            );
            let access_type = match e {
                Exception::InstructionPageFault => MemPerm::X,
                Exception::LoadPageFault => MemPerm::R,
                Exception::StorePageFault => MemPerm::X,
                _ => unreachable!(),
            };

            let addr_space = current_task().addr_space();

            if let Err(_e) = addr_space.handle_page_fault(VirtAddr::new(stval), access_type) {
                log::warn!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
                );
                log::warn!("{:x?}", current_task().trap_context_mut());
                log::warn!("bad memory access, send SIGSEGV to task");
                current_task().receive_siginfo(
                    SigInfo {
                        sig: Sig::SIGSEGV,
                        code: SigInfo::KERNEL,
                        details: SigDetails::None,
                    },
                );
            }
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_interrupt_handler(i: Interrupt, _stval: usize) {
    match i {
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
        }
        Interrupt::SupervisorTimer => {
            // log::info!("kernel SupervisorTimer enter");
            TIMER_MANAGER.check(get_time_duration());
            set_nx_timer_irq();
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_panic() -> ! {
    panic!(
        "[kernel] {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        scause::read().cause(),
        stval::read(),
        sepc::read(),
    );
}
