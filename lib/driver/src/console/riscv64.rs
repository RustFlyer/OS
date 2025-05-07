use core::fmt::{self, Write};

use mutex::SpinNoIrqLock;

use crate::CHAR_DEVICE;

pub fn console_putchar(c: u8) {
    #![allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

pub fn console_getchar() -> u8 {
    #![allow(deprecated)]
    sbi_rt::legacy::console_getchar() as u8
}

struct Stdout;

impl Write for Stdout {
    // TODO: char device support
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.as_bytes() {
            console_putchar(*s);
        }
        Ok(())
    }
}

pub fn getchar() -> u8 {
    let char_device = CHAR_DEVICE.get().unwrap();
    char_device.get()
}

pub fn console_print(args: fmt::Arguments<'_>) {
    static PRINT_MUTEX: SpinNoIrqLock<()> = SpinNoIrqLock::new(());
    let _lock = PRINT_MUTEX.lock();
    Stdout.write_fmt(args).unwrap();
}
