use loongArch64::register::estat::{self, Exception, Trap};
use loongArch64::register::{badv, ecfg, eentry, prmd, ticlr, era};

use arch::loongarch64::time::{get_time_duration, set_nx_timer_irq};
use mm::address::VirtAddr;
use timer::TIMER_MANAGER;
use crate::irq::TIMER_IRQ;
use crate::vm::mapping_flags::MappingFlags;

/// Kernel trap handler for LoongArch
/// Similar to RISC-V, this handles exceptions and interrupts that occur in kernel mode
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
        },
        _ => kernel_panic(),
    }
}

/// Handle exceptions that occur in kernel mode
pub fn kernel_exception_handler(e: Exception, badv_val: usize) {
    match e {
        // Handle TLB miss exceptions in kernel mode
        Exception::FetchPageFault | Exception::PageNonExecutableFault => {
            let access = MappingFlags::X;
            let fault_addr = VirtAddr::new(badv_val);
            if !handle_kernel_page_fault(fault_addr, access) {
                log::error!(
                    "[kernel] Instruction page fault in kernel at {:#x}, instruction = {:#x}",
                    badv_val,
                    era::read()
                );
                kernel_panic();
            }
        }
        Exception::LoadPageFault | Exception::PageNonReadableFault => {
            let access = MappingFlags::R;
            let fault_addr = VirtAddr::new(badv_val);
            if !handle_kernel_page_fault(fault_addr, access) {
                log::error!(
                    "[kernel] Load page fault in kernel at {:#x}, instruction = {:#x}",
                    badv_val,
                    era::read()
                );
                kernel_panic();
            }
        }
        Exception::StorePageFault | Exception::PageModifyFault => {
            let access = MappingFlags::W;
            let fault_addr = VirtAddr::new(badv_val);
            if !handle_kernel_page_fault(fault_addr, access) {
                log::error!(
                    "[kernel] Store page fault in kernel at {:#x}, instruction = {:#x}",
                    badv_val,
                    era::read()
                );
                kernel_panic();
            }
        }
        // Other exceptions are likely errors
        _ => {
            log::error!(
                "[kernel] {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}",
                e,
                badv_val,
                era::read()
            );
            kernel_panic();
        }
    }
}

/// Handle page faults that occur in kernel mode
/// Returns true if the page fault was handled successfully
fn handle_kernel_page_fault(fault_addr: VirtAddr, access: MappingFlags) -> bool {
    // TODO: Implement kernel page fault handling
    // In a real implementation, you might:
    // 1. Check if the address is in a memory-mapped region
    // 2. Allocate physical memory if needed
    // 3. Update kernel page tables
    // 4. Flush TLB entries
    
    log::warn!(
        "[kernel] Page fault in kernel at {:#x}, access: {:?}",
        fault_addr.to_usize(),
        access
    );
    
    // For now, we don't handle kernel page faults
    false
}

/// Handle interrupts that occur in kernel mode
pub fn kernel_interrupt_handler(irq_num: usize, _badv_val: usize) {
    match irq_num {
        // External interrupt - equivalent to SupervisorExternal in RISC-V
        // The IRQ number will be architecture-specific
        irq_num if irq_num != TIMER_IRQ => {
            log::info!("[kernel] received external interrupt: {}", irq_num);
            // TODO: Handle external interrupt in kernel mode
        }
        // Timer interrupt - handle similarly to RISC-V
        TIMER_IRQ => {
            TIMER_MANAGER.check(get_time_duration());
            ticlr::clear_timer_interrupt();
            set_nx_timer_irq();
        }
        _ => kernel_panic(),
    }
}

/// Kernel panic handler
pub fn kernel_panic() -> ! {
    panic!(
        "[kernel] {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        estat::read().cause(),
        badv::read().vaddr(),
        era::read()
    );
}