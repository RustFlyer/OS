#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use alloc::vec;

use config::process::CloneFlags;
use user_lib::{clone, exit, mmap, println, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let flags = CloneFlags::THREAD | CloneFlags::VM;
    let child_stack = mmap(0x3ff0000000 as *const u8, 0x10000, 0x3, 0x22, -1, 0);
    println!("child stack: {:#x}", child_stack);
    let ret = clone(flags.bits() as usize, child_stack, 0, 0, 0);
    let mut buf = vec![0; 32];

    // let mut rlimit = RLimit::default();
    // let _ = prlimit64(0, 3, 0, &mut rlimit);

    if ret == 0 {
        println!("child thread begin");
        for (i, v) in buf.iter_mut().enumerate() {
            println!("child thread-{}: {}", i, *v);
            *v = i;
        }
        exit(0);
    }

    for (i, v) in buf.iter_mut().enumerate() {
        println!("main thread-{}: {}", i, *v);
        *v = 100 + i;
    }

    let mut a = 0;
    waitpid(ret, &mut a);

    exit(0)
}
