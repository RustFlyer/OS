use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register,
};

use arch::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use systype::memory_flags::MappingFlags;
use timer::TIMER_MANAGER;

use crate::osdriver;
use crate::osdriver::manager::device_manager;
use crate::{
    task::{
        Task,
        signal::sig_info::{Sig, SigDetails, SigInfo},
    },
    trap::{load_trap_handler, trap_handler::TRAP_STATS},
};

/// handle exception or interrupt from a task, return if success.
/// __trap_from_user saved TrapContext, then jump to
/// the middle of trap_return(), and then return to
/// task_executor_unit(), which calls this trap_handler() function.
pub fn trap_handler(task: &Task) {
    let stval = register::stval::read();
    let sepc = register::sepc::read();
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
            user_exception_handler(task, Exception::from_number(e).unwrap(), stval, sepc)
        }
        Trap::Interrupt(i) => user_interrupt_handler(task, Interrupt::from_number(i).unwrap()),
    }
}

pub fn user_exception_handler(task: &Task, e: Exception, stval: usize, sepc: usize) {
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
                log::error!(
                    "[user_exception_handler] task [{}] {} unsolved page fault at {:#x}, \
                    access: {:?}, error: {:?}, bad instruction at {:#x}",
                    task.tid(),
                    task.get_name(),
                    fault_addr.to_usize(),
                    access,
                    e.as_str(),
                    sepc
                );
                task.receive_siginfo(SigInfo {
                    sig: Sig::SIGSEGV,
                    code: SigInfo::USER,
                    details: SigDetails::Kill {
                        pid: task.get_pgid(),
                        siginfo: None,
                    },
                });
            }
        }
        Exception::IllegalInstruction => {
            log::error!(
                "[trap_handler] illegal instruction {:#x} at {:#x}",
                stval,
                sepc
            );
            task.receive_siginfo(SigInfo {
                sig: Sig::SIGILL,
                code: SigInfo::USER,
                details: SigDetails::Kill {
                    pid: task.get_pgid(),
                    siginfo: None,
                },
            });
        }
        e => {
            log::error!("Unknown user exception: {:?}", e);
        }
    }
}

pub fn user_interrupt_handler(task: &Task, i: Interrupt) {
    // log::error!("interrupt! {:?}", i);
    match i {
        Interrupt::SupervisorTimer => {
            set_nx_timer_irq();
            TRAP_STATS.inc(i.number());
        }
        Interrupt::SupervisorExternal => {
            log::error!("[user] receive externel interrupt");
            device_manager().handle_irq();
            TRAP_STATS.inc(i.number());
        }
        _ => {
            panic!("[trap_handler] Unsupported interrupt {:?}", i);
        }
    }
}
