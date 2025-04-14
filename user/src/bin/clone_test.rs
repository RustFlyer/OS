#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, fork, println, sleep};

#[unsafe(no_mangle)]
fn main() {
    exit(-8)
}
