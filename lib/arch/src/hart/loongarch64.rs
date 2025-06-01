use config::mm::KERNEL_MAP_OFFSET;

pub fn hart_start(_hart_id: usize, _start_addr: usize) {
    // LoongArch does not require the first hart to start other harts.
    // In fact, all harts are started by the bootloader.
}

pub fn hart_shutdown() -> ! {
    // Not implemented yet
    const VIRT_POWEROFF_ADDR: *mut u32 = (KERNEL_MAP_OFFSET + 0x1000_0000) as *mut u32;
    unsafe {
        core::ptr::write_volatile(VIRT_POWEROFF_ADDR, 0x5555);
    }

    log::debug!("idle");
    loop {
        unsafe { loongArch64::asm::idle() };
    }
}
