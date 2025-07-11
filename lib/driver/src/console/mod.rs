use core::fmt::Write;

use alloc::fmt;
use polyhal_macro::define_arch_mods;

define_arch_mods!();

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            console_putchar(*c);
        }
        Ok(())
    }
}

pub fn console_print(args: fmt::Arguments<'_>) {
    Console.write_fmt(args).unwrap();
}
