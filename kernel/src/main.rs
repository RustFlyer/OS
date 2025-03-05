#![no_std]
#![no_main]

mod console;
mod lang_item;
mod sbi;

use core::arch::global_asm;

global_asm!(include_str!("entry.S"));



#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    println!("Hello, world!");
    sbi::shutdown(false);
    loop {}
}
