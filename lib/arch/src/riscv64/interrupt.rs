use riscv::interrupt;

pub fn enable_interrupt() {
    unsafe {
        interrupt::enable();
    }
}

pub fn disable_interrupt() {
    interrupt::disable();
}

pub unsafe fn set_trap_handler(handler_addr: usize) {
    stvec::write(handler_addr, TrapMode::Direct);
}