use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{satp, scause, sepc, stval},
};

use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use timer::TIMER_MANAGER;

#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(e) => exception_handler(Exception::from_number(e).unwrap(), stval),
        Trap::Interrupt(i) => interrupt_handler(Interrupt::from_number(i).unwrap(), stval),
    }
}

fn exception_handler(_e: Exception, _stval: usize) {
    trap_panic();
}

fn interrupt_handler(i: Interrupt, _stval: usize) {
    match i {
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
        }
        Interrupt::SupervisorTimer => {
            TIMER_MANAGER.check(get_time_duration());
            set_nx_timer_irq();
        }
        _ => trap_panic(),
    }
}

fn trap_panic() -> ! {
    let log = format!(
        "[kernel] panicked: cause = {:?}, bad addr = {:#x}, bad instruction = {:#x}, satp = {:#x}",
        scause::read().cause(),
        stval::read(),
        sepc::read(),
        satp::read().bits(),
    );
    log::error!("{}", log);
    panic!("{}", log);
}
