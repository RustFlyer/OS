use loongArch64::register::estat::{self, Exception, Interrupt, Trap};
use loongArch64::register::{badv, ecfg, era, pgdh, pgdl, prmd, ticlr};

use arch::{
    time::{get_time_duration, set_nx_timer_irq},
    trap::TIMER_IRQ,
};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::processor::current_task;
use crate::{task::TaskState, trap::trap_handler::TRAP_STATS};

use super::unaligned_la::emulate_load_store_insn;

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
    match _e {
        Exception::AddressNotAligned => unsafe {
            let task = current_task();
            let trap_context = task.trap_context_mut();
            emulate_load_store_insn(trap_context);
        },
        _ => (),
    }
    trap_panic();
}

fn kernel_interrupt_handler(i: Interrupt) {
    match i {
        Interrupt::Timer => {
            // log::debug!("kernel time interrupt");
            TIMER_MANAGER.check(get_time_duration());
            ticlr::clear_timer_interrupt();
            TRAP_STATS.inc(i as usize);
        }
        Interrupt::HWI0
        | Interrupt::HWI1
        | Interrupt::HWI2
        | Interrupt::HWI3
        | Interrupt::HWI4
        | Interrupt::HWI5
        | Interrupt::HWI6
        | Interrupt::HWI7 => {
            log::info!("[kernel] receive external interrupt: {:?}", i);
            TRAP_STATS.inc(i as usize);
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
