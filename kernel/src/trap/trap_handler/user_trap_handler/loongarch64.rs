use loongArch64::register::estat::{self, Exception, Interrupt, Trap};
use loongArch64::register::{badv, ecfg, eentry, prmd, ticlr};

use arch::{
    time::{get_time_duration, set_nx_timer_irq},
    trap::TIMER_IRQ,
};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::processor::current_hart;
use crate::task::{Task, TaskState};
use crate::trap::load_trap_handler;
use crate::vm::mapping_flags::MappingFlags;
use crate::vm::user_ptr::UserReadPtr;

#[unsafe(no_mangle)]
pub fn trap_handler(task: &Task) {
    let estat = estat::read();

    unsafe { load_trap_handler() };

    // Update global timer manager and check for expired timers
    let current = get_time_duration();
    TIMER_MANAGER.check(current);

    match estat.cause() {
        Trap::Exception(e) => user_exception_handler(task, e),
        Trap::Interrupt(i) => user_interrupt_handler(task, i),
        _ => {
            log::error!("Unknown trap cause");
        }
    }
}

/// Handler for user exceptions
pub fn user_exception_handler(task: &Task, e: Exception) {
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
            let fault_addr = badv::read().vaddr();
            let fault_addr = VirtAddr::new(fault_addr);
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
        Exception::InstructionNotExist => {
            let fault_addr = badv::read().vaddr();
            log::warn!("[trap_handler] illegal instruction at {:#x}", fault_addr);
            // TODO: Send SIGILL signal to the task; don't just kill the task
            task.set_state(TaskState::Zombie);
        }
        _ => {
            log::error!("Unknown user exception: {:?}", e);
        }
    }
}

/// Handler for user interrupts
pub fn user_interrupt_handler(task: &Task, i: Interrupt) {
    match i {
        Interrupt::Timer => {
            ticlr::clear_timer_interrupt();

            // If the executor does not have other tasks, no need to yield
            if task.timer_mut().schedule_time_out()
                && executor::has_waiting_task_alone(current_hart().id)
            {
                task.set_is_yield(true);
            }
        }
        _ => panic!("Unknown user interrupt: {:?}", i),
    }
}
