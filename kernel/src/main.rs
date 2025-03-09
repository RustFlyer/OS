#![no_std]
#![no_main]

mod console;
mod lang_item;
mod logging;
mod sbi;

use core::arch::global_asm;

global_asm!(include_str!("entry.S"));

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    logger::init();
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
