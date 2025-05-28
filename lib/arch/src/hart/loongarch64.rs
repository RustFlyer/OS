pub fn hart_start(_hart_id: usize, _start_addr: usize) {
    // LoongArch does not require the first hart to start other harts.
    // In fact, all harts are started by the bootloader.
}

pub fn hart_shutdown() -> ! {
    // Not implemented yet
    loop {
        unsafe { loongArch64::asm::idle() };
    }
}
