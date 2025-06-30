use loongArch64::register::estat::{self, Exception, Interrupt, Trap};
use loongArch64::register::{badv, ecfg, era, pgdh, pgdl, prmd, ticlr};

use arch::{
    time::{get_time_duration, set_nx_timer_irq},
    trap::TIMER_IRQ,
};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::task::TaskState;

#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let estat = estat::read();
    match estat.cause() {
        Trap::Exception(e) => kernel_exception_handler(e),
        Trap::Interrupt(i) => kernel_interrupt_handler(i),
        _ => trap_panic(),
    }
}

fn kernel_exception_handler(_e: Exception) {
    trap_panic();
}

fn kernel_interrupt_handler(i: Interrupt) {
    match i {
        Interrupt::Timer => {
            // log::debug!("kernel time interrupt");
            TIMER_MANAGER.check(get_time_duration());
            ticlr::clear_timer_interrupt();
        }
        _ => trap_panic(),
    }
}

fn trap_panic() -> ! {
    let msg = format!(
        "[kernel] panicked: cause = {:?}, \
        bad instruction at {:#x}, \
        fault addr (if accessing memory) = {:#x}, \
        pgdl = {:#x}, \
        pgdh = {:#x}",
        estat::read().cause(),
        era::read().raw(),
        badv::read().vaddr(),
        pgdl::read().raw(),
        pgdh::read().raw(),
    );
    crate::vm::trace_page_table_lookup(
        mm::address::PhysPageNum::new(pgdl::read().raw() >> 12),
        mm::address::VirtAddr::new(badv::read().vaddr()),
    );
    log::error!("{}", msg);
    panic!("{}", msg);
}
