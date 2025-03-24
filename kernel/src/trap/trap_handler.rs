use crate::syscall::syscall;
use crate::task::{Task, TaskState, yield_now};
use crate::trap::load_trap_handler;
use crate::vm::mem_perm::MemPerm;
use crate::vm::user_ptr::UserReadPtr;
use alloc::sync::Arc;
use arch::riscv64::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use riscv::{ExceptionNumber, InterruptNumber};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register,
};
use timer::TIMER_MANAGER;

/// handle exception or interrupt from a task, return if success.
/// __trap_from_user saved TrapContext, then jump to
/// the middle of trap_return(), and then return to
/// task_executor_unit(), which calls this trap_handler() function.
#[allow(unused)]
#[unsafe(no_mangle)]
pub async fn trap_handler(task: &Arc<Task>) -> bool {
    let stval = register::stval::read();
    let cause = register::scause::read().cause();

    simdebug::when_debug!({
        log::trace!(
            "[trap_handler] user task trap into kernel, type: {:?}, stval: {:#x}",
            cause,
            stval
        );
    });

    unsafe { load_trap_handler() };

    let current = get_time_duration();
    TIMER_MANAGER.check(current);
    set_nx_timer_irq();

    if task.timer_mut().schedule_time_out() && executor::has_waiting_task() {
        yield_now().await;
    }

    match cause {
        Trap::Exception(e) => {
            user_exception_handler(task, Exception::from_number(e).unwrap(), stval).await
        }
        Trap::Interrupt(i) => {
            user_interrupt_handler(task, Interrupt::from_number(i).unwrap()).await
        }
    }
    true
}

pub async fn user_exception_handler(task: &Arc<Task>, e: Exception, stval: usize) {
    let mut cx = task.trap_context_mut();
    match e {
        // 系统调用
        Exception::UserEnvCall => {
            let syscall_no = cx.syscall_no();
            simdebug::when_debug!({
                log::trace!("[trap_handler] user env call: syscall_no = {}", syscall_no);
            });
            cx.sepc_forward();

            let sys_ret = syscall(syscall_no, cx.syscall_args()).await;

            cx = task.trap_context_mut();
            cx.set_user_a0(sys_ret);
        }
        // 内存错误
        Exception::StorePageFault | Exception::InstructionPageFault | Exception::LoadPageFault => {
            let access = match e {
                Exception::InstructionPageFault => MemPerm::X,
                Exception::LoadPageFault => MemPerm::R,
                Exception::StorePageFault => MemPerm::W | MemPerm::R,
                _ => unreachable!(),
            };
            let fault_addr = VirtAddr::new(stval);
            if let Err(e) = task
                .addr_space_mut()
                .lock()
                .handle_page_fault(fault_addr, access)
            {
                // Should send a `SIGSEGV` signal to the task
                log::error!(
                    "[user_exception_handler] unsolved page fault at {:#x}, access: {:?}, error: {:?}",
                    fault_addr.to_usize(),
                    access,
                    e.as_str()
                );
                unimplemented!();
            }
        }
        // 非法指令
        Exception::IllegalInstruction => {
            log::warn!("[trap_handler] illegal instruction at {:#x}", stval);
            let mut addr_space_lock = task.addr_space_mut().lock();
            let mut user_ptr = UserReadPtr::<u32>::new(stval, &mut addr_space_lock);

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
        // 其他异常
        e => {
            log::warn!("Unknown user exception: {:?}", e);
        }
    }
}

pub async fn user_interrupt_handler(_task: &Arc<Task>, i: Interrupt) {
    match i {
        // 时钟中断
        Interrupt::SupervisorTimer => {
            // note: 用户若频繁陷入内核，则可能是因为时钟中断未触发，
            // 而是 supervisor 模式下触发了，导致用户程序在 CPU 上运行了很长时间。
            log::trace!("[trap_handler] timer interrupt");
            let current = get_time_duration();
            TIMER_MANAGER.check(current);
            set_nx_timer_irq();
            if executor::has_waiting_task() {
                yield_now().await;
            }
        }
        // 外部中断
        Interrupt::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
            // driver::get_device_manager_mut().handle_irq();
        }
        // 其他中断
        _ => {
            panic!("[trap_handler] Unsupported interrupt {:?}", i);
        }
    }
}
