#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]

mod console;
mod entry;
mod lang_item;
mod loader;
mod logging;
mod processor;
mod sbi;
mod task;

use mm::{self, frame, heap};
use simdebug::when_debug;

extern crate alloc;

pub fn rust_main() -> ! {
    /* Initialize logger */
    logger::init();
    when_debug!({
        log::info!("test info");
        log::error!("test error");
        log::warn!("test warn");
        log::debug!("test debug");
        log::trace!("test trace");
    });

    /* Initialize heap allocator and page table */
    unsafe {
        heap::init_heap_allocator();
        mm::enable_kernel_page_table();
    }

    when_debug!({
        log::info!(
            "RAM: {:#x} - {:#x}",
            config::mm::RAM_START,
            config::mm::RAM_END
        );

        log::info!(
            "kernel virtual memory: {:#x} - {:#x}",
            config::mm::kernel_start(),
            config::mm::kernel_end()
        );
    });

    /* Simple tests */
    when_debug!({
        heap::heap_test();
        frame::frame_alloc_test();
    });

    sbi::shutdown(false);
    loop {}
}
