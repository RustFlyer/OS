use polyhal_macro::define_arch_mods;

use config::{device::MAX_HARTS, mm::{KERNEL_STACK_SIZE, PTE_PER_TABLE}};

define_arch_mods!();

#[repr(C)]
pub struct BootStack([u8; KERNEL_STACK_SIZE * MAX_HARTS]);

#[unsafe(link_section = ".bss.stack")]
pub static mut BOOT_STACK: BootStack = BootStack([0; KERNEL_STACK_SIZE * MAX_HARTS]);

/// Boot page table, which is used temporarily before the real page table is set up.
#[repr(C, align(4096))]
struct BootPageTable([u64; PTE_PER_TABLE]);
