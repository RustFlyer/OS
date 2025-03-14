use core::arch::global_asm;

use crate::riscv64::interrupt::set_trap_handler;

global_asm!(include_str("trap.asm"));

unsafe extern "C" {
    fn __trap_from_kernel();
}

pub fn set_sepc() {
    set_trap_handler(__trap_from_kernel as usize)
}