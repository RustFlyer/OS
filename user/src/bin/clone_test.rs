#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, fork, wait};

#[unsafe(no_mangle)]
fn main() {
    let mut buf: [u8; 128] = [0; 128];
    let pid = fork();

    if pid == 0 {
        for b in 0..128 {
            buf[b] = 8;
        }
        exit(0)
    }

    for b in 0..128 {
        buf[b] = 1;
    }
    let mut t = 0;
    wait(&mut t);
    exit(-8)
}
