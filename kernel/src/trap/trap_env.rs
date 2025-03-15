use core::arch::global_asm;

use arch::riscv64::interrupt::set_trap_handler;

global_asm!(include_str!("trap.asm"));

unsafe extern "C" {
    fn __trap_from_kernel();
    fn __trap_from_user();
}

pub fn set_kernel_stvec() {
    unsafe { set_trap_handler(__trap_from_kernel as usize) };
}

pub fn set_user_stvec() {
    unsafe { set_trap_handler(__trap_from_user as usize) };
}
