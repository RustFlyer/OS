#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

mod console;
mod lang_item;
mod loader;
mod logging;
mod processor;
mod sbi;
mod task;

use core::arch::global_asm;

extern crate alloc;
extern crate mm;

global_asm!(include_str!("entry.S"));

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    logger::init();
    log::info!("test info");
    log::error!("test error");
    log::warn!("test warn");
    log::debug!("test debug");
    log::trace!("test trace");
    simdebug::when_debug!({
        log::info!("when debug");
    });
    log::info!(
        "kernel physical memory: {:#x} - {:#x}",
        config::mm::KERNEL_START_PHYS,
        config::mm::_ekernel as usize
    );
    log::info!(
        "kernel virtual memory: {:#x} - {:#x}",
        config::mm::KERNEL_START,
        config::mm::_ekernel as usize + config::mm::KERNEL_VM_OFFSET
    );

    unsafe { mm::heap::init_heap_allocator() };

    sbi::shutdown(false);
    loop {}
}
