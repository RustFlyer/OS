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

use core::arch::global_asm;

use mm::{
    frame, heap,
    vm::page_table::{self, PageTable},
};

extern crate alloc;

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    /* Initialize logger */
    logger::init();
    simdebug::when_debug!({
        log::info!("--------when debug--------");
        log::info!("test info");
        log::error!("test error");
        log::warn!("test warn");
        log::debug!("test debug");
        log::trace!("test trace");
        log::info!("--------when debug--------");
    });

    /* Initialize heap allocator and page table */
    unsafe {
        heap::init_heap_allocator();
        page_table::enable_kernel_page_table();
    }

    log::info!(
        "RAM: {:#x} - {:#x}",
        config::mm::RAM_START,
        config::mm::RAM_END
    );

    log::info!(
        "kernel physical memory: {:#x} - {:#x}",
        config::mm::KERNEL_START_PHYS,
        config::mm::kernel_end_phys(),
    );

    log::info!(
        "kernel virtual memory: {:#x} - {:#x}",
        config::mm::KERNEL_START,
        config::mm::kernel_end() as usize
    );

    /* Simple tests */
    simdebug::when_debug!({
        heap::heap_test();
        frame::frame_alloc_test();
    });

    sbi::shutdown(false);
    loop {}
}
