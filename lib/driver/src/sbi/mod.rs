use super::{CHAR_DEVICE, CharDevice};
use config::sbi::*;
use core::arch::asm;
use core::fmt::{self, Write};
use mutex::SpinNoIrqLock;

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

pub fn console_getchar() -> u8 {
    sbi_call(SBI_CONSOLE_GETCHAR, 0, 0, 0) as u8
}

pub fn clear_ipi() -> usize {
    sbi_call(SBI_CLEAR_IPI, 0, 0, 0)
}

pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, timer, 0, 0);
}

pub struct SbiChar;

impl SbiChar {
    pub fn new() -> Self {
        Self {}
    }
}

impl CharDevice for SbiChar {
    fn get(&self) -> u8 {
        console_getchar()
    }
    fn puts(&self, str: &[u8]) {
        for s in str {
            console_putchar(*s as usize);
        }
    }
    fn handle_irq(&self) {
        todo!()
    }
}

struct Stdout;

impl Write for Stdout {
    // TODO: char device support
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.as_bytes() {
            console_putchar(*s as usize);
        }
        Ok(())
    }
}

pub fn getchar() -> u8 {
    let char_device = CHAR_DEVICE.get().unwrap();
    char_device.get()
}

pub fn sbi_print(args: fmt::Arguments<'_>) {
    static PRINT_MUTEX: SpinNoIrqLock<()> = SpinNoIrqLock::new(());
    let _lock = PRINT_MUTEX.lock();
    Stdout.write_fmt(args).unwrap();
}
