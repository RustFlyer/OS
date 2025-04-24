#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let mut i = 0;
    if fork() == 0 {
        execve("busybox", &["busybox", "sh"], &[]);
        // execve("shell", &[], &[]);
    } else {
        loop {
            waitpid(-1, &mut i);
        }
    }
}
