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

/// handle exception or interrupt from a task, return if success.
/// Similar to the RISC-V implementation, this function is called after
/// the trap context is saved and we're ready to handle the trap.
#[unsafe(no_mangle)]
pub fn trap_handler(task: &Task) -> bool {
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
            false
        }
    }
    true
}

/// Handler for user exceptions
pub fn user_exception_handler(task: &Task, e: Exception, badv_val: usize) -> bool {
    match e {
        Exception::Syscall => {
            task.set_is_syscall(true);
            true
        }
        // LoongArch specific TLB miss exceptions - these correspond to page faults in RISC-V
        Exception::FetchPageFault | Exception::PageNonExecutableFault => {
            let access = MappingFlags::X;
            let fault_addr = VirtAddr::new(badv_val);
            handle_page_fault(task, fault_addr, access)
        }
        Exception::LoadPageFault | Exception::PageNonReadableFault => {
            let access = MappingFlags::R;
            let fault_addr = VirtAddr::new(badv_val);
            handle_page_fault(task, fault_addr, access)
        }
        Exception::StorePageFault | Exception::PageModifyFault => {
            let access = MappingFlags::W;
            let fault_addr = VirtAddr::new(badv_val);
            handle_page_fault(task, fault_addr, access)
        }
        Exception::Breakpoint => {
            // Handle breakpoint exceptions
            // Note: In LoongArch, we might need to adjust PC similar to RISC-V
            // For now, just log the event
            log::debug!("Breakpoint exception at address: {:#x}", badv_val);
            true
        }
        Exception::AddressNotAligned => {
            log::warn!("[trap_handler] address not aligned at {:#x}", badv_val);
            // TODO: Implement proper handling for unaligned access
            // Could potentially emulate the instruction like in polyhal
            task.set_state(TaskState::Zombie);
            false
        }
        Exception::IllegalInstruction => {
            log::warn!("[trap_handler] illegal instruction at {:#x}", badv_val);
            // Read the illegal instruction - this is similar to RISC-V code
            let addr_space = task.addr_space();
            let mut user_ptr = UserReadPtr::<u32>::new(badv_val, &addr_space);

            // TODO: Set MXR bit equivalent if needed in LoongArch
            // SAFETY: Reading the instruction that caused the fault
            let inst = unsafe { user_ptr.read().unwrap_or(0) };
            log::warn!("The illegal instruction is {:#x}", inst);

            task.set_state(TaskState::Zombie);
            false
        }
        _ => {
            log::warn!("Unknown user exception: {:?}", e);
            false
        }
    }
}

/// Helper function for handling page faults
fn handle_page_fault(task: &Task, fault_addr: VirtAddr, access: MappingFlags) -> bool {
    let addr_space = task.addr_space();

    if let Err(e) = addr_space.handle_page_fault(fault_addr, access) {
        // Should send a `SIGSEGV` signal to the task
        log::error!(
            "[user_exception_handler] unsolved page fault at {:#x}, access: {:?}, error: {:?}",
            fault_addr.to_usize(),
            access,
            e.as_str()
        );
        // TODO: Implement proper signal handling
        return false;
    }
    true
}

/// Handler for user interrupts
pub fn user_interrupt_handler(task: &Task, irq_num: usize) -> bool {
    match irq_num {
        TIMER_IRQ => {
            // Handle timer interrupt - similar to RISC-V
            ticlr::clear_timer_interrupt();
            set_nx_timer_irq();

            // If executor doesn't have other tasks, no need to yield
            if task.timer_mut().schedule_time_out()
                && executor::has_waiting_task_alone(current_hart().id)
            {
                task.set_is_yield(true);
            }
            true
        }
        // External interrupt handling - similar to supervisor external in RISC-V
        // The actual IRQ number may differ in LoongArch
        _ => {
            log::info!("[trap_handler] Received external interrupt: {}", irq_num);
            // TODO: Implement proper device IRQ handling
            // driver::get_device_manager_mut().handle_irq();
            true
        }
    }
}
