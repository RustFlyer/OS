// use crate::sbi::console_putchar;
use core::fmt::{self, Write};

use mutex::SpinNoIrqLock;

static PRINT_LOCK: SpinNoIrqLock<()> = SpinNoIrqLock::new(());

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            driver::console::console_putchar(*c);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    let _guard = PRINT_LOCK.lock();
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
