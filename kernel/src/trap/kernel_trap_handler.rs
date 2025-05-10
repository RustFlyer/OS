use mm::address::{PhysPageNum, VirtAddr};
use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{satp, scause, sepc, stval},
};

use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use timer::TIMER_MANAGER;

use crate::vm::trace_page_table_lookup;

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
    let root = satp::read().ppn();
    log::error!("Page table entry at 0x3fffffd000:");
    trace_page_table_lookup(PhysPageNum::new(root), VirtAddr::new(0x3fffffd000));
    log::error!("Page table entry at 0x3fffffe000:");
    trace_page_table_lookup(PhysPageNum::new(root), VirtAddr::new(0x3fffffe000));
    log::error!("Page table entry at 0x3ffffff000:");
    trace_page_table_lookup(PhysPageNum::new(root), VirtAddr::new(0x3ffffff000));
    log::error!(
        "[kernel] {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}, satp = {:#x}",
        e,
        stval,
        sepc::read(),
        satp::read().bits(),
    );
    kernel_panic();
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
