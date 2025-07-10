use riscv::register::sie;

pub fn enable_external_interrupt() {
    unsafe {
        sie::set_sext();
    }
}
