use loongArch64::register::estat::{self, Exception, Trap};
use loongArch64::register::{badv, ecfg, eentry, prmd, ticlr};

use arch::loongarch64::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::irq::TIMER_IRQ;
use crate::processor::current_hart;
use crate::task::{Task, TaskState};
use crate::trap::load_trap_handler;
use crate::vm::mapping_flags::MappingFlags;
use crate::vm::user_ptr::UserReadPtr;

#[unsafe(no_mangle)]
pub fn trap_handler(task: &Task) {
    let badv_val = badv::read().vaddr();
    let estat_val = estat::read();
    let cause = estat_val.cause();

    unsafe { load_trap_handler() };

    // Update global timer manager and check for expired timers
    let current = get_time_duration();
    TIMER_MANAGER.check(current);

    match cause {
        Trap::Exception(e) => user_exception_handler(task, e, badv_val),
        Trap::Interrupt(_) => {
            // Get the IRQ number from estat register
            let irq_num: usize = estat_val.is().trailing_zeros() as usize;
            user_interrupt_handler(task, irq_num)
        }
        _ => {
            log::error!("Unknown trap cause");
        }
    }
}

/// Handler for user exceptions
pub fn user_exception_handler(task: &Task, e: Exception, badv_val: usize) {
    match e {
        Exception::Syscall => {
            task.set_is_syscall(true);
        }
        Exception::FetchPageFault
        | Exception::PageNonExecutableFault
        | Exception::LoadPageFault
        | Exception::PageNonReadableFault
        | Exception::StorePageFault
        | Exception::PageModifyFault => {
            let access = match e {
                Exception::FetchPageFault | Exception::PageNonExecutableFault => MappingFlags::X,
                Exception::LoadPageFault | Exception::PageNonReadableFault => MappingFlags::R,
                Exception::StorePageFault | Exception::PageModifyFault => MappingFlags::W,
                _ => unreachable!(),
            };
            let fault_addr = VirtAddr::new(badv_val);
            let addr_space = task.addr_space();
            if let Err(e) = addr_space.handle_page_fault(fault_addr, access) {
                // TODO: Send SIGSEGV to the task
                log::error!(
                    "[user_exception_handler] unsolved page fault at {:#x}, access: {:?}, error: {:?}",
                    fault_addr.to_usize(),
                    access,
                    e.as_str()
                );
                task.set_state(TaskState::Zombie);
            }
        }
        Exception::IllegalInstruction => {
            log::warn!("[trap_handler] illegal instruction at {:#x}", badv_val);
            // TODO: Send SIGILL signal to the task; don't just kill the task
            task.set_state(TaskState::Zombie);
        }
        _ => {
            log::error!("Unknown user exception: {:?}", e);
        }
    }
}

/// Handler for user interrupts
pub fn user_interrupt_handler(task: &Task, irq_num: usize) {
    match irq_num {
        TIMER_IRQ => {
            ticlr::clear_timer_interrupt();
            set_nx_timer_irq();

            // If executor doesn't have other tasks, no need to yield
            if task.timer_mut().schedule_time_out()
                && executor::has_waiting_task_alone(current_hart().id)
            {
                task.set_is_yield(true);
            }
        }
        _ => {
            log::info!("[trap_handler] Received external interrupt: {}", irq_num);
            // TODO: Implement proper device IRQ handling
            // driver::get_device_manager_mut().handle_irq();
        }
    }
}
