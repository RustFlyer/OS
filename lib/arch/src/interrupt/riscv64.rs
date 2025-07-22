use riscv::register::sie;

pub fn enable_external_interrupt() {
    unsafe {
        sie::set_sext();
    }
}

pub fn is_interrupt_on() -> bool {
    riscv::register::sstatus::read().sie()
}
