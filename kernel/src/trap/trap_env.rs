use core::arch::global_asm;

use arch::riscv64::interrupt::set_trap_handler;
use riscv::register::stvec::TrapMode;

global_asm!(include_str!("trap.asm"));

unsafe extern "C" {
    fn __trap_from_kernel();
    fn __trap_from_user();
    fn __user_rw_trap_vector();
}

pub fn set_kernel_stvec() {
    unsafe { set_trap_handler(__trap_from_kernel as usize, TrapMode::Direct) };
}

pub fn set_user_stvec() {
    unsafe { set_trap_handler(__trap_from_user as usize, TrapMode::Direct) };
}

pub fn set_kernel_stvec_user_rw() {
    set_trap_handler(__user_rw_trap_vector as usize, TrapMode::Vectored);
}
