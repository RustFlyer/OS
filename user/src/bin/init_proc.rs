#![no_std]
#![no_main]

extern crate user_lib;

use core::ptr::NonNull;

use user_lib::{exit, println, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let mut a: u128 = 0;
    let mut i = 0;
    loop {
        waitpid(0, &mut i);
    }

    exit(0)
}
