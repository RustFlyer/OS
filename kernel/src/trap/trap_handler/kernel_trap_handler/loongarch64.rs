use loongArch64::register::estat::{self, Exception, Trap};
use loongArch64::register::{badv, ecfg, eentry, era, prmd, ticlr};

use arch::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::irq::TIMER_IRQ;
use crate::vm::mapping_flags::MappingFlags;

#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let estat_val = estat::read();
    let badv_val = badv::read().vaddr();
    match estat_val.cause() {
        Trap::Exception(e) => kernel_exception_handler(e, badv_val),
        Trap::Interrupt(_) => {
            // Get the IRQ number from estat register
            let irq_num: usize = estat_val.is().trailing_zeros() as usize;
            kernel_interrupt_handler(irq_num, badv_val)
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_exception_handler(e: Exception, badv_val: usize) {
    kernel_panic();
}

pub fn kernel_interrupt_handler(irq_num: usize, _badv_val: usize) {
    match irq_num {
        irq_num if irq_num != TIMER_IRQ => {
            log::info!("[kernel] received external interrupt: {}", irq_num);
        }
        TIMER_IRQ => {
            TIMER_MANAGER.check(get_time_duration());
            ticlr::clear_timer_interrupt();
            set_nx_timer_irq();
        }
        _ => kernel_panic(),
    }
}

pub fn kernel_panic() -> ! {
    log = format!(
        "[kernel] {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        estat::read().cause(),
        badv::read().vaddr(),
        era::read()
    );
    log::error!("{}", log);
    panic!("{}", log);
}
