use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{
        scause::{self},
        sepc, stval,
    },
};
use timer::TIMER_MANAGER;

/// Kernel trap handler
#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let _stval = stval::read();
    match scause.cause() {
        Trap::Exception(e) => kernel_exception_handler(Exception::from_number(e).unwrap()),
        Trap::Interrupt(i) => kernel_interrupt_handler(Interrupt::from_number(i).unwrap()),
    }
}

pub fn kernel_exception_handler(e: Exception) {
    match e {
        Exception::StorePageFault => kernel_panic(),
        _ => log::error!("Something Wrong Happen: {:?}", e),
    }
}

pub fn kernel_interrupt_handler(i: Interrupt) {
    match i {
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
        }
        Interrupt::SupervisorTimer => {
            TIMER_MANAGER.check(get_time_duration());
            set_nx_timer_irq();
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_panic() -> ! {
    panic!(
        "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        scause::read().cause(),
        stval::read(),
        sepc::read(),
    );
}
