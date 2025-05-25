use core::arch::global_asm;

use arch::trap::{set_trap_handler, TrapMode};

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("rv_trap.s"));
#[cfg(target_arch = "loongarch64")]
global_asm!(include_str!("loong_trap.s"));

unsafe extern "C" {
    fn __trap_from_kernel();
    fn __trap_from_user();
    fn __user_rw_trap_vector();
}

pub fn set_kernel_trap_entry() {
    set_trap_handler(__trap_from_kernel as usize, TrapMode::Direct);
}

pub fn set_user_trap_entry() {
    set_trap_handler(__trap_from_user as usize, TrapMode::Direct);
}

/// Set the trap vector as such:
/// 1. the handler of interrupts is the same as normal condition, but
/// 2. the handler of exceptions is changed into `__user_rw_exception_entry`, which
///    will “returns” 1 to indicate that an exception occurred in supervisor mode,
///    which is used to check the permission of the user memory access.
///
/// # Safety
/// This function is safe by itself, but the special trap vector set by this function
/// should be only enabled in the context of user memory access permission checking.
/// After the check is done, the trap vector should be restored to the normal one
/// before any other operations.
pub fn set_user_rw_trap_entry() {
    set_trap_handler(__user_rw_trap_vector as usize, TrapMode::Vectored);
}

 