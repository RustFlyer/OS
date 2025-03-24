#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, fork, println, sleep, yield_};

#[unsafe(no_mangle)]
fn main() {
    let mut a: i32 = 0;

    fork();

    for i in 0..=10 {
        a = a + i;
        println!("thread apple: {}", i);
        sleep(1000);
    }

    exit(880008800)
}
