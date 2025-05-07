use core::arch::asm;

use config::mm::{USER_END, USER_START};

pub fn switch_pagetable(satp: usize) {
    unsafe {
        asm!(
            "csrw satp, {}",
            "sfence.vma",
            in(reg) satp
        );
    }
}

pub fn fence() {
    riscv::asm::fence();
}

pub fn fence_i() {
    riscv::asm::fence_i();
}

pub fn sfence_vma_all() {
    riscv::asm::sfence_vma_all();
}

pub fn sfence_vma_all_except_global() {
    unsafe {
        asm!("sfence.vma x0, {0}", in(reg) 0);
    }
}

pub fn sfence_vma_addr(addr: usize) {
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

/// TLB shootdown for the whole address space (except global mappings).
pub fn tlb_shootdown_all() {
    sbi_rt::remote_sfence_vma_asid(
        sbi_rt::HartMask::from_mask_base(0, usize::MAX),
        USER_START,
        USER_END - USER_START,
        0,
    );
}
