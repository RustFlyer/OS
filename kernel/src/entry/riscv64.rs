// This module is adapted from Phoenix OS project.
// The original code is licensed under MIT License.
// The original code can be found at https://github.com/djphoenix/phoenix-os.

//! Module for the entry point of the kernel.
//!
//! This module contains the entry point of the kernel, `_start`, which is the first part
//! of code that runs after the bootloader. The `_start` function enables Sv39 page table,
//! sets up the stack pointer, and jumps to the `rust_main` function, thus jumping to the
//! kernel's virtual memory space.
//!
//! It uses a minimal page table with two huges page entries, which map to the same physical
//! address region, to make the control flow transition to the virtual address space smoothly.
//! Subsequent code should set up the real page table and switch to it as soon as possible.
//!
//! A fixed-size stack, `BOOT_STACK`, is used as the stack the kernel runs on. The stack size
//! should be large enough, but a too large stack size is a waste of memory.

use core::arch::naked_asm;

use config::mm::KERNEL_MAP_OFFSET;

use super::{BOOT_STACK, BootPageTable};
use crate::rust_main;

/// The boot page table contains the following two huge page entries:
/// 0x0000_0000_8000_0000 -> 0x0000_0000_8000_0000
/// 0xffff_ffc0_8000_0000 -> 0x0000_0000_8000_0000
static mut BOOT_PAGE_TABLE: BootPageTable = {
    let mut arr: [u64; 512] = [0; 512];
    // Flags: VRWXAD
    arr[2] = (0x80000 << 10) | 0xcf;
    arr[258] = (0x80000 << 10) | 0xcf;
    BootPageTable(arr)
};

#[naked]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start(hart_id: usize, dtb_addr: usize) -> ! {
    // Note: The `hart_id` parameter is passed in `a0` register on boot; do not overwrite it here.
    unsafe {
        naked_asm!(
            // Enable Sv39 page table
            // satp = (8 << 60) | ppn
            "
            la      t0, {page_table_pa}
            srli    t0, t0, 12              // t0 = ppn of page table
            li      t1, 8 << 60
            or      t0, t0, t1
            csrw    satp, t0
            sfence.vma
        ",
            // Kernel address offset between physical and virtual memory
            "
            li      t0, {kernel_map_offset}
        ",
            // Set stack pointer to the virtual address of the upper bound of the boot stack
            "
            addi    t1, a0, 1
            slli    t1, t1, 16              // t1 = (hart_id + 1) * KERNEL_STACK_SIZE
            la      sp, {boot_stack_pa}
            add     sp, sp, t1
            add     sp, sp, t0
        ",
            // Jump to the virtual address of `rust_main`
            "
            la      a2, {rust_main}
            or      a2, a2, t0
            jr      a2
        ",
            page_table_pa = sym BOOT_PAGE_TABLE,
            boot_stack_pa = sym BOOT_STACK,
            kernel_map_offset = const KERNEL_MAP_OFFSET,
            rust_main = sym rust_main,
        )
    }
}
