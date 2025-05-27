use core::arch::naked_asm;

use loongArch64::register::estat::{self, Exception, Interrupt, Trap};
use loongArch64::register::{
    badv, ecfg, era, prmd, pwch, pwcl, stlbps, ticlr, tlbidx, tlbrehi, tlbrentry,
};

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
        other_cause => {
            log::error!("Unknown trap cause: {:?}", other_cause);
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
            let fault_addr = VirtAddr::new(badv::read().vaddr());
            let addr_space = task.addr_space();
            if let Err(e) = addr_space.handle_page_fault(fault_addr, access) {
                // TODO: Send SIGSEGV to the task
                log::error!(
                    "[user_exception_handler] unsolved page fault at {:#x}, \
                    access: {:?}, error: {:?}, bad instruction at {:#x}",
                    fault_addr.to_usize(),
                    access,
                    e.as_str(),
                    era::read().pc()
                );
                task.set_state(TaskState::Zombie);
            }
        }
        Exception::InstructionNotExist => {
            let inst_addr = era::read().pc();
            log::warn!("[trap_handler] illegal instruction at {:#x}", inst_addr);
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
            // log::debug!("user time interrupt");
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

/// Set up CSRs to configure the TLB and page table.
///
/// This function sets the page size, the layout of virtual addresses, the entry address
/// of the TLB refill exception handler, and some other settings related to the TLB
/// and page table.
///
/// This function must be called exactly once during system initialization, before
/// enabling paging mechanism.
///
/// # About the Layout of virtual addresses
/// The “layout” of virtual addresses refers to how the software/hardware page table
/// walking mechanism interprets the virtual address.
///
/// We use a 3-level page table with 4 KiB pages. The virtual address is divided as
/// follows:
/// - Bits 0-11: Offset within a page (4 KiB page).
/// - Bits 12-20: Index into the level 0 page table.
/// - Bits 21-29: Index into the level 1 page table.
/// - Bits 30-38: Index into the level 3 page table.
pub fn tlb_init() {
    const PAGE_SIZE_EXP: usize = 12; // 4 KiB page = 2^12 bytes

    // See LoongArch Reference Manual 5.4.5 to know the meaning of these constants.
    const PT_BASE: usize = 12;
    const PT_WIDTH: usize = 9;
    const DIR1_BASE: usize = PT_BASE + PT_WIDTH;
    const DIR1_WIDTH: usize = 9;
    const DIR3_BASE: usize = DIR1_BASE + DIR1_WIDTH;
    const DIR3_WIDTH: usize = 9;

    stlbps::set_ps(PAGE_SIZE_EXP);
    tlbidx::set_ps(PAGE_SIZE_EXP);
    tlbrehi::set_ps(PAGE_SIZE_EXP);
    pwcl::set_pte_width(8); // 64-bit page table entry

    // Set the layout of virtual addresses
    pwcl::set_ptbase(PT_BASE);
    pwcl::set_ptwidth(PT_WIDTH);
    pwcl::set_dir1_base(DIR1_BASE);
    pwcl::set_dir1_width(DIR1_WIDTH);
    pwch::set_dir3_base(DIR3_BASE);
    pwch::set_dir3_width(DIR3_WIDTH);

    tlbrentry::set_tlbrentry(tlb_refill as usize);
}

/// The TLB refill exception handler.
///
/// The control flow goes here when a TLB refill exception occurs.
///
/// This function walks the current page table to find the page table entries
/// corresponding to the faulting virtual address, and fills the TLB with the
/// entries.
#[naked]
pub unsafe extern "C" fn tlb_refill() {
    unsafe {
        naked_asm!(
            "
            .equ LA_CSR_PGDL,          0x19    /* Page table base address when VA[47] = 0 */
            .equ LA_CSR_PGDH,          0x1a    /* Page table base address when VA[47] = 1 */
            .equ LA_CSR_PGD,           0x1b    /* Page table base */
            .equ LA_CSR_TLBRENTRY,     0x88    /* TLB refill exception entry */
            .equ LA_CSR_TLBRBADV,      0x89    /* TLB refill badvaddr */
            .equ LA_CSR_TLBRERA,       0x8a    /* TLB refill ERA */
            .equ LA_CSR_TLBRSAVE,      0x8b    /* KScratch for TLB refill exception */
            .equ LA_CSR_TLBRELO0,      0x8c    /* TLB refill entrylo0 */
            .equ LA_CSR_TLBRELO1,      0x8d    /* TLB refill entrylo1 */
            .equ LA_CSR_TLBREHI,       0x8e    /* TLB refill entryhi */
            .balign 4096
                csrwr   $t0, LA_CSR_TLBRSAVE
                csrrd   $t0, LA_CSR_PGD
                lddir   $t0, $t0, 3
                lddir   $t0, $t0, 1
                ldpte   $t0, 0
                ldpte   $t0, 1
                tlbfill
                csrrd   $t0, LA_CSR_TLBRSAVE
                ertn
            "
        );
    }
}
