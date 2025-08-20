use loongArch64::register::{crmd, ecfg, eentry};

use crate::interrupt::enable_external_interrupt;

use super::TrapMode;

/// Timer IRQ of loongarch64
pub const TIMER_IRQ: usize = 11;

pub fn init() {
    enable_interrupt();
    enable_external_interrupt();
}

pub fn enable_interrupt() {
    // log::error!("enable_interrupt");
    crmd::set_ie(true);
}

pub fn disable_interrupt() {
    // log::error!("disable_interrupt");
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
    let vs = match mode {
        TrapMode::Direct => 0,
        TrapMode::Vectored => 1,
    };
    // Modifying the `ecfg` and `eentry` registers must be done atomically.
    let old_ie = crmd::read().ie();
    disable_interrupt();
    ecfg::set_vs(vs);
    eentry::set_eentry(handler_addr);
    if old_ie {
        enable_interrupt();
    } else {
        disable_interrupt();
    }
}
