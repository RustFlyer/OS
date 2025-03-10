#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

mod console;
mod lang_item;
mod loader;
mod logging;
mod sbi;
mod task;
use core::arch::global_asm;

extern crate alloc;
extern crate mm;

global_asm!(include_str!("entry.S"));

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    logger::init();
    unsafe { mm::heap::init_heap_allocator() };
    log::info!("Hello, world!");
    log::error!("test error");
    log::warn!("test warn");
    log::debug!("test debug");
    log::trace!("test trace");
    log::info!("when debug2");
    simdebug::when_debug!({
        log::info!("when debug");
        log::info!("when debug");
        log::info!("when debug");
    });
    sbi::shutdown(false);
    loop {}
}
