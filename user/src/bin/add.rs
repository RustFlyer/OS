#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, println, yield_};

#[unsafe(no_mangle)]
fn main() {
    let mut a: i32 = 0;

    for i in 0..=100 {
        a = a + i;
        println!("thread1: {}", i);
        if i % 8 == 0 {
            yield_();
        }
    }

    exit(a)
}
