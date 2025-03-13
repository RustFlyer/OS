#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]

mod boot;
mod console;
mod entry;
mod lang_item;
mod loader;
mod logging;
mod processor;
mod sbi;
mod task;
mod trap;

use core::{arch::global_asm, sync::atomic::AtomicBool};

use mm::{
    frame, heap,
    vm::page_table::{self, PageTable},
};

use core::sync::atomic::Ordering;

extern crate alloc;

static MAIN_HART: AtomicBool = AtomicBool::new(true);

#[unsafe(no_mangle)]
pub fn rust_main(hart_id: usize) -> ! {
    println!("hart {} is running", hart_id);
    if MAIN_HART
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        /* Initialize logger */
        logger::init();
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

        simdebug::when_debug!({
            log::info!("start harts");
            boot::start_harts(hart_id);
        });
    } else {
        log::info!("hart {} is waiting", hart_id);
    }

    log::info!("hart {} is running", hart_id);

    loop {}
    sbi::shutdown(false);
}
