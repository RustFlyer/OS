use config::mm::KERNEL_MAP_OFFSET;
use crate::trap::disable_interrupt;
use mm::address::PhysAddr;

// HALT_ADDR is the virtual address of Generic Event Device (GED) in Qemu-LoongArch64
// GED only enabled when -machine virt is used in Qemu
const HALT_ADDR: *mut u8 = (KERNEL_MAP_OFFSET + 0x100E001C) as *mut u8;

pub fn hart_start(_hart_id: usize, _start_addr: usize) {
    // LoongArch does not require the first hart to start other harts.
    // In fact, all harts are started by the bootloader.
}

/// Shutdown the whole system and all harts.
pub fn hart_shutdown() -> ! {
    log::info!("Shutting down whole system");
    // 0x34 is the magic number to shutdown the system
    unsafe { core::ptr::write_volatile(HALT_ADDR, 0x34) };
    disable_interrupt();
    unsafe { loongArch64::asm::idle() }
    log::warn!("It should have shutdowned");
    loop {
        disable_interrupt();
        unsafe { loongArch64::asm::idle() }
    }
}
