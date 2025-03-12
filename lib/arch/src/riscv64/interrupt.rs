use riscv::interrupt;

pub fn enable_interrupt() {
    unsafe {
        interrupt::enable();
    }
}

pub fn disable_interrupt() {
    interrupt::disable();
}
