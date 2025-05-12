use loongArch64::register::{crmd, ecfg, eentry};

use super::TrapMode;

/// Timer IRQ of loongarch64
pub const TIMER_IRQ: usize = 11;

pub fn init() {
    // Enable interrupt
    crmd::set_ie(true);
}

pub fn enable_interrupt() {
    crmd::set_ie(true);
}

pub fn disable_interrupt() {
    crmd::set_ie(false);
}

/// Set the trap handler.
///
/// # Note for LoongArch64
/// LoongArch64 does not have multiple trap modes like RISC-V. Yet, we can still consider
/// that the LoongArch64 architecture supports two trap modes: direct and vectored. In
/// direct mode, given the address of the trap handler, the CPU will jump directly to that
/// address when a trap occurs. In vectored mode, given an address, the CPU will jump to an
/// address which is calculated by `handler_addr + ecode * interval`, where `ecode` is the
/// exception code or interrupt code of the trap type, and `interval` is set by the `ecfg`
/// register. In our implementation, we should have `interval` equal to the size of an jump
/// instruction, and each handler should be just a jump instruction to the real handler.
pub fn set_trap_handler(handler_addr: usize, mode: TrapMode) {
    let inerval = match mode {
        TrapMode::Direct => 0,
        TrapMode::Vectored => 1,
    };
    ecfg::set_vs(inerval);
    eentry::set_eentry(handler_addr);
}
