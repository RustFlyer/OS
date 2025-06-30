#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, fork, println, sleep};

#[unsafe(no_mangle)]
fn main() {
    let mut a: i32 = 0;

    if fork() == 0 {
        if fork() == 0 {
            println!("PKL PKL FTT FTT AHC AHC");
        }
    }

    println!("fork begin to run!");

    for i in 0..=10 {
        a = a + i;
        println!("thread apple: {}", i);
        sleep(1000);
    }

    exit(880008800)
}
