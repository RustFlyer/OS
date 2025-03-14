use config::sbi::*;
use core::arch::asm;

#[inline(always)]
fn sbi_call(eid_fid: (usize, usize), arg0: usize, arg1: usize, arg2: usize) -> usize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") arg0 => ret,
            in("x11") arg1,
            in("x12") arg2,
            in("x16") eid_fid.1,
            in("x17") eid_fid.0,
        );
    }
    ret
}

pub fn hart_start(hart_id: usize, start_addr: usize) -> usize {
    sbi_call(SBI_HART_START, hart_id, start_addr, 0)
}

pub fn hart_shutdown(failure: bool) -> ! {
    sbi_call(SBI_SHUTDOWN, failure as usize, 0, 0);
    panic!("hart shutdown failed");
}

pub fn console_putchar(c: usize) -> usize {
    sbi_call(SBI_CONSOLE_PUTCHAR, c, 0, 0)
}

pub fn console_getchar() -> usize {
    sbi_call(SBI_CONSOLE_GETCHAR, 0, 0, 0)
}

pub fn clear_ipi() -> usize {
    sbi_call(SBI_CLEAR_IPI, 0, 0, 0)
}
