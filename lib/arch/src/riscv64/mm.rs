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
