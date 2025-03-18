#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::exit;

#[unsafe(no_mangle)]
fn main() {
    let mut a: i32 = 0;

    for i in 0..=(3 << 12) {
        a = a + i;
    }

    exit(a)
}
