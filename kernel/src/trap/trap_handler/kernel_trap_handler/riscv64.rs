use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{satp, scause, sepc},
};

use arch::time::{get_time_duration, set_nx_timer_irq};
use timer::TIMER_MANAGER;

#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(e) => exception_handler(Exception::from_number(e).unwrap()),
        Trap::Interrupt(i) => interrupt_handler(Interrupt::from_number(i).unwrap()),
    }
}

fn exception_handler(_e: Exception) {
    trap_panic();
}

fn interrupt_handler(i: Interrupt) {
    match i {
        Interrupt::SupervisorTimer => {
            TIMER_MANAGER.check(get_time_duration());
            set_nx_timer_irq();
        }
        _ => trap_panic(),
    }
}

fn trap_panic() -> ! {
    let msg = format!(
        "[kernel] panicked: cause = {:?}, \
        bad instruction at {:#x}, \
        fault addr (if accessing memory) = {:#x}, \
        satp = {:#x}",
        scause::read().cause(),
        sepc::read(),
        stval::read(),
        satp::read().bits(),
    );
    log::error!("{}", msg);
    panic!("{}", msg);
}
