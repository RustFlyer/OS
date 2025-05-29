#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use alloc::vec;
use config::process::CloneFlags;
use user_lib::{RLimit, clone, exit, println, prlimit64, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let flags = CloneFlags::THREAD | CloneFlags::VM;
    let ret = clone(flags.bits() as usize, 0x3fffffc000, 0, 0, 0);
    let mut buf = vec![0; 32];

    // let mut rlimit = RLimit::default();
    // let _ = prlimit64(0, 3, 0, &mut rlimit);

    if ret == 0 {
        println!("child thread begin");
        for i in 0..32 {
            println!("child thread-{}: {}", i, buf[i]);
            buf[i] = 32 + i;
        }
        exit(0);
    }

    for i in 0..32 {
        println!("main thread-{}: {}", i, buf[i]);
        buf[i] = 32 + i;
    }

    let mut a = 0;
    waitpid(ret, &mut a);

    exit(0)
}
