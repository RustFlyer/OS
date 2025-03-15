#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]
#![allow(dead_code, unused_imports, warnings)]

mod boot;
mod console;
mod entry;
mod lang_item;
mod loader;
mod logging;
mod processor;
mod sbi;
mod syscall;
mod task;
mod trap;

use core::sync::atomic::Ordering;
use core::{arch::global_asm, sync::atomic::AtomicBool};

use mm::{self, frame, heap};
use simdebug::when_debug;

pub use syscall::syscall;

extern crate alloc;

static mut INITIALIZED: bool = false;

#[unsafe(no_mangle)]
pub fn rust_main(hart_id: usize) -> ! {
    // SAFETY: Only the first hart will run this code block.
    if unsafe { !INITIALIZED } {
        /* Initialize logger */
        logger::init();
        log::info!("hart {}: initializing kernel", hart_id);

        /* Initialize heap allocator and page table */
        unsafe {
            log::info!("hart {}: initializing heap allocator", hart_id);
            heap::init_heap_allocator();
            log::info!("hart {}: initializing page table", hart_id);
            mm::enable_kernel_page_table();
            INITIALIZED = true;
        }

        log::info!("======== kernel memory layout ========");
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
        log::info!(".text {:#x} - {:#x}", config::mm::text_start(), config::mm::text_end());
        log::info!(".rodata {:#x} - {:#x}", config::mm::rodata_start(), config::mm::rodata_end());
        log::info!(".data {:#x} - {:#x}", config::mm::data_start(), config::mm::data_end());
        log::info!(".bss {:#x} - {:#x}", config::mm::bss_start(), config::mm::bss_end());
        log::info!("====== kernel memory layout end ======");

        boot::start_harts(hart_id);

        when_debug!({
            simdebug::backtrace_test();
        });
    } else {
        log::info!("hart {}: enabling page table", hart_id);
        // SAFETY: Only after the first hart has initialized the heap allocator and page table,
        // do the other harts enable the kernel page table.
        unsafe { mm::enable_kernel_page_table(); }
    }

    log::info!("hart {}: running", hart_id);

    loop {
        // executor::task_run_always();
    }
    sbi::shutdown(false);
}
