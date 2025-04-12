#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let mut i = 0;
    if fork() == 0 {
        execve("shell\0", &[], &[]);
    } else {
        loop {
            waitpid(0, &mut i);
        }
    }
}
