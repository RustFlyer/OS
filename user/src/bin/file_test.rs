#![no_std]
#![no_main]

extern crate user_lib;

use config::{inode::InodeMode, vfs::OpenFlags};
use user_lib::{exit, fork, open, println, sleep, yield_};

#[unsafe(no_mangle)]
fn main() {
    let _: isize = open(0, "aaa", OpenFlags::O_CREAT, InodeMode::empty());

    exit(9)
}
