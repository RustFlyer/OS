use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register,
};

use arch::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use systype::memory_flags::MappingFlags;
use timer::TIMER_MANAGER;

use crate::processor::current_hart;
use crate::task::{Task, TaskState};
use crate::trap::load_trap_handler;
use crate::vm::user_ptr::UserReadPtr;

/// handle exception or interrupt from a task, return if success.
/// __trap_from_user saved TrapContext, then jump to
/// the middle of trap_return(), and then return to
/// task_executor_unit(), which calls this trap_handler() function.
pub fn trap_handler(task: &Task) {
    let stval = register::stval::read();
    let cause = register::scause::read().cause();

    unsafe { load_trap_handler() };

    // Here task updates global timer manager and checks if there
    // are any expired timer. If there is, the task will wake up
    // the relevant thread.
    // to ensure that timer check is stably called, the kernel
    // spawns a timer kernel thread [`time_init`] to do this.
    let current = get_time_duration();
    TIMER_MANAGER.check(current);

    match cause {
        Trap::Exception(e) => {
            user_exception_handler(task, Exception::from_number(e).unwrap(), stval)
        }
        Trap::Interrupt(i) => user_interrupt_handler(task, Interrupt::from_number(i).unwrap()),
    }
}

pub fn user_exception_handler(task: &Task, e: Exception, stval: usize) {
    match e {
        Exception::UserEnvCall => {
            task.set_is_syscall(true);
        }
        Exception::StorePageFault | Exception::InstructionPageFault | Exception::LoadPageFault => {
            let access = match e {
                Exception::InstructionPageFault => MappingFlags::X,
                Exception::LoadPageFault => MappingFlags::R,
                Exception::StorePageFault => MappingFlags::W,
                _ => unreachable!(),
            };
            let fault_addr = VirtAddr::new(stval);
            let addr_space = task.addr_space();
            if let Err(e) = addr_space.handle_page_fault(fault_addr, access) {
                // TODO: Send SIGSEGV to the task
                log::error!(
                    "[user_exception_handler] task [{}] {} unsolved page fault at {:#x}, \
                    access: {:?}, error: {:?}, bad instruction at {:#x}",
                    task.tid(),
                    task.get_name(),
                    fault_addr.to_usize(),
                    access,
                    e.as_str(),
                    stval
                );
                task.set_state(TaskState::Zombie);
            }
        }
        Exception::IllegalInstruction => {
            log::warn!("[trap_handler] illegal instruction at {:#x}", stval);
            // TODO: Send SIGILL signal to the task; don't just kill the task
            task.set_state(TaskState::Zombie);
        }
        e => {
            log::error!("Unknown user exception: {:?}", e);
        }
    }
}

pub fn user_interrupt_handler(task: &Task, i: Interrupt) {
    match i {
        Interrupt::SupervisorTimer => {
            set_nx_timer_irq();

            // If the executor does not have other tasks, no need to yield.
            if task.timer_mut().schedule_time_out()
                && executor::has_waiting_task_alone(current_hart().id)
            {
                log::trace!(
                    "[trap_handler] task {} yield, contain signal: {:?}",
                    task.tid(),
                    task.sig_manager_mut().bitmap.bits()
                );
                task.set_is_yield(true);
            }
        }
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
            // driver::get_device_manager_mut().handle_irq();
        }
        _ => {
            panic!("[trap_handler] Unsupported interrupt {:?}", i);
        }
    }
}
