#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, println, yield_};

#[unsafe(no_mangle)]
fn main() {
    let mut a: i32 = 0;

    for i in 0..=30 {
        a = a + i;
        println!("thread banana: {}", i);
    }

    exit(114514)
}
