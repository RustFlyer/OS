use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use riscv::register::{
    scause::{self, Exception, Interrupt, Scause, Trap},
    sepc, stval, stvec,
};
use timer::TIMER_MANAGER;

/// Kernel trap handler
#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let _stval = stval::read();
    match scause.cause() {
        Trap::Interrupt(i) => kernel_interrupt_handler(i),
        _ => kernel_panic(),
    }
}

pub fn kernel_interrupt_handler(i: Interrupt) {
    match i {
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
        }
        Interrupt::SupervisorTimer => {
            TIMER_MANAGER.check(get_time_duration());
            unsafe { set_nx_timer_irq() };
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_panic() -> ! {
    panic!(
        "[kernel] {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        scause::read().cause(),
        scause::read().bits(),
        stval::read(),
        sepc::read(),
    );
}
