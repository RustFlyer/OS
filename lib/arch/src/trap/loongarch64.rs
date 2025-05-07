use loongArch64::register::{crmd, prmd};

/// Timer IRQ of loongarch64
pub const TIMER_IRQ: usize = 11;

pub fn enable_interrupt() {
    crmd::set_ie(true);
}

pub fn disable_interrupt() {
    crmd::set_ie(false);
}

// pub fn set_trap_handler(handler_addr: usize, mode: TrapMode) {
    // unsafe {
        // let mut stvec = Stvec::from_bits(0);
        // stvec.set_address(handler_addr);
        // stvec.set_trap_mode(mode);
        // stvec::write(stvec);
    // }
// }
