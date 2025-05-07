use riscv::interrupt;
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec::{self, Stvec};

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

pub fn set_trap_handler(handler_addr: usize, mode: TrapMode) {
    unsafe {
        let mut stvec = Stvec::from_bits(0);
        stvec.set_address(handler_addr);
        stvec.set_trap_mode(mode);
        stvec::write(stvec);
    }
}
