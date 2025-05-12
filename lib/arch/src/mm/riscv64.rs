use core::arch::asm;

use config::mm::{USER_END, USER_START};
use riscv::register::satp::{self, Satp};

/// Switches the current page table being used by the MMU to the one
/// at the given address `root`.
pub fn switch_pagetable(root: usize) {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(riscv::register::satp::Mode::Sv39);
    satp.set_ppn(root >> 12);
    unsafe {
        satp::write(satp);
    }
    // The board only supports ASID=0, so we need to flush the TLB.
    tlb_flush_all_except_global();
}

pub fn fence() {
    riscv::asm::fence();
}

pub fn fence_i() {
    riscv::asm::fence_i();
}

pub fn tlb_flush_all() {
    riscv::asm::sfence_vma_all();
}

pub fn tlb_flush_all_except_global() {
    unsafe {
        asm!("sfence.vma x0, {0}", in(reg) 0);
    }
}

pub fn tlb_flush_addr(addr: usize) {
    riscv::asm::sfence_vma(0, addr);
}

/// TLB shootdown for the specified address range.
pub fn tlb_shootdown(addr: usize, length: usize) {
    sbi_rt::remote_sfence_vma_asid(
        sbi_rt::HartMask::from_mask_base(0, usize::MAX),
        addr,
        length,
        0,
    );
}

/// TLB shootdown for the whole user address space.
pub fn tlb_shootdown_all() {
    sbi_rt::remote_sfence_vma_asid(
        sbi_rt::HartMask::from_mask_base(0, usize::MAX),
        USER_START,
        USER_END - USER_START,
        0,
    );
}
