use core::arch::asm;

use loongArch64::register::pgdl;

use crate::pte::PageTableEntry;

/// Switches the current page table being used by the MMU to the one
/// at the given physical page number `root`.
pub fn switch_page_table(root: usize) {
    pgdl::set_base(root << 12);
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

/// Fills the TLB with the given page table entries for the current faulting
/// virtual address.
///
/// `vpn` is the virtual page number of `pte0` or `pte1`. `pte0` and `pte1` are
/// two page table entries that is to be filled into the TLB.
///
/// This function must be called when the kernel is handling a page fault.
pub fn tlb_fill(pte0: PageTableEntry, pte1: PageTableEntry) {
    let tlbelo0 = pte0.bits();
    let tlbelo1 = pte1.bits();
    unsafe {
        asm!(
            "
            csrwr {}, 0x12
            csrwr {}, 0x13
            tlbfill
            ",
            in(reg) tlbelo0,
            in(reg) tlbelo1,
        )
    }
}
