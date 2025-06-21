#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, println};

#[unsafe(no_mangle)]
fn main() {
    let mut _a: i32 = 0;

    for i in 0..=30 {
        _a += i;
        println!("thread orange: {}", i);
    }

    println!("orange complete");
    exit(77248)
}
