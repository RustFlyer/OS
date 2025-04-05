#![no_std]
#![no_main]

extern crate user_lib;

use config::{inode::InodeMode, vfs::OpenFlags};
use user_lib::{exit, fork, lseek, open, println, read, sleep, write, yield_};

#[unsafe(no_mangle)]
fn main() {
    let fd_aaa: isize = open(
        0,
        "tes",
        OpenFlags::O_CREAT | OpenFlags::O_RDWR,
        InodeMode::empty(),
    );
    println!("file test: fd of tes is [{}]", fd_aaa);

    let write_text = "Moon rises at night";
    write(fd_aaa as usize, write_text.as_bytes());
    println!("file test: finish write");

    lseek(fd_aaa as usize, 0, 0);
    println!("file test: lseek to origin");

    let mut read_buf: [u8; 1024] = [0; 1024];
    read(fd_aaa as usize, &mut read_buf);
    let utf2str = core::str::from_utf8(&read_buf).unwrap();
    println!("file test: read text [{}]", utf2str);

    exit(9)
}
