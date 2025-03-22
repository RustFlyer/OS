use core::arch::asm;

pub fn switch_pagetable(satp: usize) {
    unsafe {
        asm!(
            "csrw satp, {}",
            "sfence.vma",
            in(reg) satp
        );
    }
}

pub fn sfence_vma_all() {
    unsafe {
        core::arch::riscv64::sfence_vma_all();
    }
}
