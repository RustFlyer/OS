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

/// TLB shootdown for the specified address range.
///
/// # TODO
/// This function currently only flushes the TLB for the current hart.
/// In the future, it should be extended to support real TLB shootdown
/// mechanism for LoongArch64.
pub fn tlb_shootdown(_addr: usize, _length: usize) {
    tlb_flush_all_except_global();
    // TODO: Implement TLB shootdown mechanism for LoongArch64.
}

/// TLB shootdown for the whole user address space.
///
/// # TODO
/// This function currently only flushes the TLB for the current hart.
/// In the future, it should be extended to support real TLB shootdown
pub fn tlb_shootdown_all() {
    tlb_flush_all_except_global();
    // TODO: Implement TLB shootdown mechanism for LoongArch64.
}
