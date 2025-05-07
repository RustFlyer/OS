use loongArch64::ipi::{csr_mail_send, send_ipi_single};

// Note: I have no idea what how this code works.
pub fn hart_start(hart_id: usize, sp_top: usize) {
    csr_mail_send(crate::components::boot::_start_secondary as _, hart_id, 0);
    csr_mail_send(sp_top as _, hart_id, 1);
    send_ipi_single(1, 1);
}

pub fn hart_shutdown() -> ! {
    log::warn!("Shutting down on loongarch64 platform was not implemented!");
    loop {
        unsafe { loongArch64::asm::idle() };
    }
}
