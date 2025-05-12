use loongArch64::ipi::{csr_mail_send, send_ipi_single};

pub fn hart_start(hart_id: usize, start_addr: usize) {
    // Note: I have no idea what this code does.
    csr_mail_send(start_addr as u64, hart_id, 0);
    send_ipi_single(1, 1);
}

pub fn hart_shutdown() -> ! {
    // Not implemented yet
    loop {
        unsafe { loongArch64::asm::idle() };
    }
}
