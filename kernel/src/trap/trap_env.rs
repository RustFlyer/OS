use core::arch::global_asm;

use arch::riscv64::interrupt::set_trap_handler;
use riscv::register::stvec::TrapMode;
use simdebug::when_debug;

use crate::vm::user_ptr::{try_read, try_write};

global_asm!(include_str!("rv_trap.asm"));

unsafe extern "C" {
    fn __trap_from_kernel();
    fn __trap_from_user();
    fn __user_rw_trap_vector();
}

pub fn set_kernel_stvec() {
    set_trap_handler(__trap_from_kernel as usize, TrapMode::Direct);
}

pub fn set_user_stvec() {
    set_trap_handler(__trap_from_user as usize, TrapMode::Direct);
}

/// Set the trap vector as such:
/// 1. the handler of interrupts is the same as normal condition, but
/// 2. the handler of exceptions is changed into `__user_rw_exception_entry`, which
///    will “returns” 1 to indicate that an exceptions is caused in supervisor mode,
///    which is used to check the permission of the user memory access.
///
/// # Safety
/// This function is safe by itself, but the special trap vector set by this function
/// should be only enabled in the context of user memory access permission checking.
/// After the check is done, the trap vector should be restored to the normal one
/// before any other operations.
pub fn set_kernel_stvec_user_rw() {
    set_trap_handler(__user_rw_trap_vector as usize, TrapMode::Vectored);
}

// TODO: Could be safer if match scause.(not for loongArch)
pub fn will_read_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert_eq!(curr_stvec, __user_rw_trap_vector as usize);
    });
    // when try_read failed, it return false, and will_read_fail should return true
    !unsafe{ try_read(vaddr) }
}

pub fn will_write_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert!(curr_stvec == __user_rw_trap_vector as usize);
    });
    !unsafe{ try_write(vaddr) }
}