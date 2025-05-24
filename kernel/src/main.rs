#![no_std]
#![no_main]
#![feature(btree_cursors)]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]
#![allow(clippy::module_inception)]
#![allow(dead_code)]

mod boot;
mod entry;
mod lang_item;
mod link_app;
mod loader;
mod logging;
mod osdriver;
mod processor;
mod syscall;
mod task;
mod trap;
mod vm;

use core::ptr;

use arch::mm::fence;
use config::mm::{DTB_END, DTB_START};
use mm::{self, frame, heap};
use processor::hart;

#[macro_use]
extern crate alloc;

static mut INITIALIZED: bool = false;

pub fn rust_main(hart_id: usize, dtb_addr: usize) -> ! {
    executor::init(hart_id);

    // SAFETY: Only the first hart will run this code block.
    if unsafe { !INITIALIZED } {
        /* Initialize logger */
        logger::init();
        log::info!("hart {}: initializing kernel", hart_id);
        log::info!("dtb_addr: {:#x}", dtb_addr);

        /* Initialize heap allocator and page table */
        unsafe {
            heap::init_heap_allocator();
            log::info!("hart {}: initialized heap allocator", hart_id);
            frame::init_frame_allocator();
            log::info!("hart {}: initialized frame allocator", hart_id);
            vm::switch_to_kernel_page_table();
            log::info!("hart {}: switched to kernel page table", hart_id);
            fence();
            ptr::write_volatile(&raw mut INITIALIZED, true);
        }

        log::info!(
            "kernel physical memory: {:#x} - {:#x}",
            config::mm::KERNEL_START_PHYS,
            config::mm::kernel_end_phys(),
        );
        log::info!(
            "kernel virtual memory: {:#x} - {:#x}",
            config::mm::KERNEL_START,
            config::mm::kernel_end()
        );
        log::info!(
            ".text {:#x} - {:#x}",
            config::mm::text_start(),
            config::mm::text_end()
        );
        log::info!(
            ".rodata {:#x} - {:#x}",
            config::mm::rodata_start(),
            config::mm::rodata_end()
        );
        log::info!(
            ".data {:#x} - {:#x}",
            config::mm::data_start(),
            config::mm::data_end()
        );
        log::info!(
            ".bss {:#x} - {:#x}",
            config::mm::bss_start(),
            config::mm::bss_end()
        );
        log::info!("device tree blob {:#x} - {:#x}", DTB_START, DTB_END,);
        log::info!("device tree blob PA start: {:#x}", dtb_addr);
        log::info!("====== kernel memory layout end ======");

        unsafe {
            config::mm::DTB_ADDR = dtb_addr;
            osdriver::probe_tree();
        }

        driver::init();
        log::info!("hart {}: initialized driver", hart_id);

        // block_device_test();

        osfs::init();
        log::info!("hart {}: initialized FS success", hart_id);

        // boot::start_harts(hart_id);

        // loader::init();

        task::init();
    } else {
        log::info!("hart {}: enabling page table", hart_id);
        // SAFETY: Only after the first hart has initialized the heap allocator and page table,
        // do the other harts enable the kernel page table.
        unsafe {
            vm::switch_to_kernel_page_table();
        }
    }

    arch::trap::init();

    #[cfg(target_arch = "loongarch64")]
    trap::trap_handler::tlb_init();

    hart::init(hart_id);

    log::info!("hart {}: running", hart_id);

    loop {
        executor::task_run_always_alone(hart_id);
    }
}
