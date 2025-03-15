use riscv::interrupt;
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec::{self, Stvec};

pub fn enable_interrupt() {
    unsafe {
        interrupt::enable();
    }
}

pub fn disable_interrupt() {
    interrupt::disable();
}

pub unsafe fn set_trap_handler(handler_addr: usize) {
    unsafe {
        let mut stvec = Stvec::from_bits(0);
        stvec.set_address(handler_addr);
        stvec.set_trap_mode(TrapMode::Direct);
        stvec::write(stvec);
    }
}
