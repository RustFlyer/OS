use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register,
};

use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;

use crate::processor::current_hart;
use crate::task::{Task, TaskState};
use crate::trap::load_trap_handler;
use crate::vm::mem_perm::MemPerm;
use crate::vm::user_ptr::UserReadPtr;

/// handle exception or interrupt from a task, return if success.
/// __trap_from_user saved TrapContext, then jump to
/// the middle of trap_return(), and then return to
/// task_executor_unit(), which calls this trap_handler() function.
#[unsafe(no_mangle)]
pub fn trap_handler(task: &Task) {
    let stval = register::stval::read();
    let cause = register::scause::read().cause();

    unsafe { load_trap_handler() };

    // log::info!("[trap_handler] enter");

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
                Exception::InstructionPageFault => MemPerm::X,
                Exception::LoadPageFault => MemPerm::R,
                Exception::StorePageFault => MemPerm::W,
                _ => unreachable!(),
            };
            let fault_addr = VirtAddr::new(stval);
            let addr_space = task.addr_space();
            // log::debug!("pass sleep lock {:?}", addrspace.change_heap_size(0, 0));
            if let Err(e) = addr_space.handle_page_fault(fault_addr, access) {
                // Should send a `SIGSEGV` signal to the task

                log::error!(
                    "[user_exception_handler] task [{}] {} unsolved page fault at {:#x}, access: {:?}, error: {:?}",
                    task.tid(),
                    task.get_name(),
                    fault_addr.to_usize(),
                    access,
                    e.as_str()
                );
                unimplemented!();
            }
        }
        Exception::IllegalInstruction => {
            log::warn!("[trap_handler] illegal instruction at {:#x}", stval);
            let addr_space = task.addr_space();
            let mut user_ptr = UserReadPtr::<u32>::new(stval, &addr_space);

            let old_sstatus = register::sstatus::read();
            unsafe {
                register::sstatus::set_mxr();
            }
            // SAFETY: the instruction must reside in a `X` page since it's an instruction fetch,
            // and we have set `MXR` bit in `sstatus` register.
            let inst = unsafe { user_ptr.read().unwrap() };
            unsafe { register::sstatus::write(old_sstatus) };

            log::warn!("The illegal instruction is {:#x}", inst);
            task.set_state(TaskState::Zombie);
        }
        e => {
            log::warn!("Unknown user exception: {:?}", e);
        }
    }
}

pub fn user_interrupt_handler(task: &Task, i: Interrupt) {
    // log::error!("[trap_handler] user_interrupt_handler");
    match i {
        Interrupt::SupervisorTimer => {
            // log::trace!("[trap_handler] timer interrupt");
            set_nx_timer_irq();

            // if executor does not have other tasks, it is no need to yield.
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
