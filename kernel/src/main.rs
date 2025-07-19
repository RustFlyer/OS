#![no_std]
#![no_main]
#![feature(btree_cursors)]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]
#![allow(clippy::module_inception)]
#![feature(stmt_expr_attributes)]
#![feature(slice_as_array)]
#![feature(core_intrinsics)]
// #![allow(dead_code)]
// #![allow(unused)]

mod boot;
mod entry;
mod lang_item;
mod loader;
mod logging;
mod net;
mod osdriver;
mod processor;
mod syscall;
mod task;
mod trap;
mod vm;

use core::ptr;

use arch::{
    mm::{fence, tlb_flush_all},
    trap::disable_interrupt,
};
use config::mm::{DTB_ADDR, DTB_END, DTB_START};
use driver::println;
use logging::{disable_log, enable_log};
use mm::{self, frame, heap};

#[macro_use]
extern crate alloc;

static mut INITIALIZED: bool = false;

pub static NIGHTHAWK_OS_BANNER: &str = r#"
  _   _ _       _     _   _                    _     ____   _____ 
 | \ | (_)     | |   | | | |                  | |   / __ \ / ____|
 |  \| |_  __ _| |__ | |_| |__   __ ___      _| | _| |  | | (___  
 | . ` | |/ _` | '_ \| __| '_ \ / _` \ \ /\ / / |/ / |  | |\___ \ 
 | |\  | | (_| | | | | |_| | | | (_| |\ V  V /|   <| |__| |____) |
 |_| \_|_|\__, |_| |_|\__|_| |_|\__,_| \_/\_/ |_|\_\\____/|_____/ 
           __/ |                                                  
          |___/                                                   
             NighthawkOS
"#;

pub fn rust_main(hart_id: usize, dtb_addr: usize) -> ! {
    disable_interrupt();
    println!("hart id: {}, dtb_addr: {:#x}", hart_id, dtb_addr);
    executor::init(hart_id);
    logger::init();
    enable_log();

    // SAFETY: Only the first hart will run this code block.
    if unsafe { !INITIALIZED } {
        println!("print init");
        /* Initialize logger */

        println!("hart id: {}, dtb_addr: {:#p}", hart_id, &dtb_addr);
        boot::clear_bss();

        // too much log delay, cut up!
        disable_log();
        println!("hart id: {}, dtb_addr: {:#p}", hart_id, &dtb_addr);

        println!("disable_log");

        log::info!("hart {}: initializing kernel", hart_id);
        log::info!("dtb_addr: {:#x}", dtb_addr);

        #[cfg(target_arch = "loongarch64")]
        log::warn!("ARCH: loongarch64");

        #[cfg(target_arch = "riscv64")]
        log::warn!("ARCH: riscv64");

        println!("mem init");
        /* Initialize heap allocator and page table */
        unsafe {
            DTB_ADDR = dtb_addr;

            println!("try to init heap");

            heap::init_heap_allocator();
            log::info!("hart {}: initialized heap allocator", hart_id);
            println!("init_heap_allocator");

            frame::init_frame_allocator();
            log::info!("hart {}: initialized frame allocator", hart_id);
            println!("init_frame_allocator");

            vm::switch_to_kernel_page_table();
            log::info!("hart {}: switched to kernel page table", hart_id);
            println!("switch_to_kernel_page_table");

            fence();
            ptr::write_volatile(&raw mut INITIALIZED, true);
        }
        println!("memory init success");

        println!(
            "kernel physical memory: {:#x} - {:#x}",
            config::mm::KERNEL_START_PHYS,
            config::mm::kernel_end_phys(),
        );
        println!(
            "kernel virtual memory: {:#x} - {:#x}",
            config::mm::KERNEL_START,
            config::mm::kernel_end()
        );
        println!(
            ".text {:#x} - {:#x}",
            config::mm::text_start(),
            config::mm::text_end()
        );
        println!(
            ".rodata {:#x} - {:#x}",
            config::mm::rodata_start(),
            config::mm::rodata_end()
        );
        println!(
            ".data {:#x} - {:#x}",
            config::mm::data_start(),
            config::mm::data_end()
        );
        println!(
            ".bss {:#x} - {:#x}",
            config::mm::bss_start(),
            config::mm::bss_end()
        );
        log::info!("device tree blob {:#x} - {:#x}", DTB_START, DTB_END,);
        log::info!("device tree blob PA start: {:#x}", dtb_addr);
        log::info!("====== kernel memory layout end ======");

        println!("[PROBE_DEV_TREE] try to init");
        osdriver::probe_device_tree();
        println!("[PROBE_DEV_TREE] INIT SUCCESS");

        println!("device init");

        log::info!("hart {}: initialized driver", hart_id);

        osfs::init();
        log::info!("hart {}: initialized FS success", hart_id);
        println!("[FILE_SYSTEM] INIT SUCCESS");

        // boot::start_harts(hart_id);
        loader::init();
        syscall::init_key();

        task::init();
        println!("[USER_APP] INIT SUCCESS");
        println!("[HART {}] INIT SUCCESS", hart_id);
        println!("{}", NIGHTHAWK_OS_BANNER);
    } else {
        log::info!("hart {}: enabling page table", hart_id);
        // SAFETY: Only after the first hart has initialized the heap allocator and page table,
        // do the other harts enable the kernel page table.
        unsafe {
            vm::switch_to_kernel_page_table();
            config::board::HARTS_NUM += 1;
        }
        println!("[HART {}] INIT SUCCESS", hart_id);
        panic!("multi-core unsupported");
    }

    #[cfg(target_arch = "loongarch64")]
    trap::trap_handler::tlb_init();

    arch::trap::init();
    trap::trap_env::set_kernel_trap_entry();
    arch::time::init_timer();

    // hart::init(hart_id);
    log::info!("hart {}: running", hart_id);
    loop {
        executor::task_run_always_alone(hart_id);
    }
}
