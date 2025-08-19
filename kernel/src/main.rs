#![no_std]
#![no_main]
#![feature(btree_cursors)]
#![feature(naked_functions)]
#![feature(sync_unsafe_cell)]
#![allow(clippy::module_inception)]
#![feature(stmt_expr_attributes)]
#![feature(slice_as_array)]
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

use core::{ptr, sync::atomic::AtomicBool};

use arch::mm::{fence, tlb_flush_all};
use config::mm::{DTB_ADDR, DTB_END, DTB_START};
use driver::{block::block_test, println};
use logging::{disable_log, enable_filter, enable_log};
use mm::{self, frame, heap};
use osdriver::test_serial_output;

#[macro_use]
extern crate alloc;

static mut UNINITIALIZED: bool = true;

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
    // when single core

    // SAFETY: Only the first hart will run this code block.
    if unsafe { UNINITIALIZED } {
        // println!("print init");
        /* Initialize logger */

        boot::clear_bss();

        // too much log delay, cut up!
        logger::init();
        enable_log();
        // disable_log();

        // println!("hart id: {}, dtb_addr: {:#x}", hart_id, dtb_addr);

        // println!(
        //     "kernel physical memory start at: {:#p}",
        //     &config::mm::KERNEL_START_PHYS,
        // );

        // log::info!("dtb_addr: {:#x}", dtb_addr);

        // #[cfg(target_arch = "loongarch64")]
        // log::warn!("ARCH: loongarch64");

        // #[cfg(target_arch = "riscv64")]
        // log::warn!("ARCH: riscv64");

        // println!("mem init");
        /* Initialize heap allocator and page table */

        unsafe {
            DTB_ADDR = dtb_addr;

            println!("try to init heap");

            heap::init_heap_allocator();
            log::info!("hart {}: initialized heap allocator", hart_id);

            frame::init_frame_allocator();
            log::info!("hart {}: initialized frame allocator", hart_id);

            vm::switch_to_kernel_page_table();
            log::info!("hart {}: switched to kernel page table", hart_id);

            fence();
            ptr::write_volatile(&raw mut UNINITIALIZED, false);

            trap::trap_env::set_kernel_trap_entry();

            #[cfg(target_arch = "loongarch64")]
            trap::trap_handler::tlb_init();

            arch::trap::init();
            arch::time::init_timer();
        }

        println!(
            "kernel physical memory start at: {:#p}",
            &config::mm::KERNEL_START_PHYS,
        );

        println!("memory init");

        println!(
            "kernel physical memory: {:#x} - {:#x}",
            config::mm::KERNEL_START_PHYS,
            config::mm::kernel_end_phys(),
        );

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

        executor::init(hart_id);

        println!("[USER_APP] INIT SUCCESS");
        println!("[HART {}] INIT SUCCESS", hart_id);
        println!("{}", NIGHTHAWK_OS_BANNER);
    } else {
        executor::init(hart_id);
        log::info!("hart {}: enabling page table", hart_id);
        // SAFETY: Only after the first hart has initialized the heap allocator and page table,
        // do the other harts enable the kernel page table.
        unsafe {
            vm::switch_to_kernel_page_table();
        }
        arch::trap::init();
        trap::trap_env::set_kernel_trap_entry();
        arch::time::init_timer();
        println!("[HART {}] INIT SUCCESS", hart_id);

        #[cfg(target_arch = "loongarch64")]
        trap::trap_handler::tlb_init();

        panic!("multi-core unsupported");
    }

    // vfs::path::test_split_parent_and_name();
    // hart::init(hart_id);
    log::info!("hart {}: running", hart_id);
    osfuture::block_on(async { test_serial_output().await });
    println!("Begin to run shell..");
    enable_log();
    task::init();
    loop {
        executor::task_run_always_alone(hart_id);
    }
}
