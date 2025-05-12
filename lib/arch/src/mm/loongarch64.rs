use core::arch::asm;

use loongArch64::register::pgdl;

/// Switches the current page table being used by the MMU to the one
/// at the given address `root`.
pub fn switch_pagetable(root: usize) {
    pgdl::set_base(root);
    tlb_flush_all_except_global();
}

pub fn fence() {
    unsafe {
        asm!("dbar 0");
    }
}

pub fn fence_i() {
    unsafe {
        asm!("ibar 0");
    }
}

pub fn tlb_flush_all() {
    unsafe {
        asm!("invtlb 0x0, $r0, $r0");
    }
}

pub fn tlb_flush_all_except_global() {
    tlb_flush_all();
}

pub fn tlb_flush_addr(addr: usize) {
    unsafe {
        asm!(
            "invtlb 0x5, $r0, {reg}",
            reg = in(reg) addr
        );
    }
}
