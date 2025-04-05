#![no_std]
#![no_main]

extern crate user_lib;

use config::{inode::InodeMode, vfs::OpenFlags};
use user_lib::{exit, fork, open, println, read, sleep, write, yield_};

#[unsafe(no_mangle)]
fn main() {
    let fd_aaa: isize = open(0, "add", OpenFlags::O_CREAT, InodeMode::empty());

    println!("file test: fd of add is [{}]", fd_aaa);

    let write_text = "Moon rises at night";

    write(fd_aaa as usize, write_text.as_bytes());

    println!("file test: finish write");
    let mut read_buf: [u8; 1024] = [0; 1024];

    read(fd_aaa as usize, &mut read_buf);

    println!("file test: read text {:?}", read_buf);

    exit(9)
}
