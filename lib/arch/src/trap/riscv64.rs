use riscv::interrupt;
use riscv::register::mtvec::TrapMode as RiscvTrapMode;
use riscv::register::stvec::{self, Stvec};

use super::TrapMode;

impl From<TrapMode> for RiscvTrapMode {
    fn from(mode: TrapMode) -> Self {
        match mode {
            TrapMode::Direct => RiscvTrapMode::Direct,
            TrapMode::Vectored => RiscvTrapMode::Vectored,
        }
    }
}

pub fn init() {
    unsafe {
        // Enable timer interrupt
        riscv::register::sie::set_stimer();
        // Enable interrupt
        riscv::register::sstatus::set_sie();
    }
}

pub fn enable_interrupt() {
    unsafe {
        interrupt::enable();
    }
}

pub fn disable_interrupt() {
    interrupt::disable();
}

/// Set the trap handler.
///
/// # Note for RISC-V
/// The RISC-V architecture supports two modes of handling traps: direct and vectored.
/// In direct mode, given the address of the trap handler, the CPU will jump directly
/// to that address when a trap occurs. In vectored mode, given the address of a vector
/// of addresses of trap handlers, the CPU will jump to an address in the vector
/// corresponding to the trap type.
pub fn set_trap_handler(handler_addr: usize, mode: TrapMode) {
    unsafe {
        let mut stvec = Stvec::from_bits(0);
        stvec.set_address(handler_addr);
        stvec.set_trap_mode(mode.into());
        stvec::write(stvec);
    }
}
