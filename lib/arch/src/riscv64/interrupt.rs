use riscv::interrupt;
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec;

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
        stvec::write(handler_addr, TrapMode::Direct);
    }
}

pub fn set_stvec(handler_addr: usize) {
    unsafe {
        stvec::write(handler_addr, TrapMode::Direct);
    }
}
