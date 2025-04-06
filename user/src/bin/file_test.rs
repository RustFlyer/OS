#![no_std]
#![no_main]

extern crate user_lib;

use config::{inode::InodeMode, vfs::OpenFlags};
use user_lib::{exit, fork, lseek, open, println, read, sleep, write, yield_};

#[unsafe(no_mangle)]
fn main() {
    let fd: isize = open(
        0,
        "tes",
        OpenFlags::O_CREAT | OpenFlags::O_RDWR,
        InodeMode::REG,
    );
    println!("file test: fd of tes is [{}]", fd);

    let write_text = "Moon rises at night. Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night. Moon rises at night.Moon rises at night. Moon rises at night. Moon rises at night. Moon rises at night.Moonaa";
    // let write_text = "Moon rises at night. Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night. Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night. Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.Moon rises at night.";
    write(fd as usize, write_text.as_bytes());
    println!("file test: finish write");

    lseek(fd as usize, 0, 0);
    println!("file test: lseek to origin");

    let mut read_buf: [u8; 1024] = [0; 1024];
    read(fd as usize, &mut read_buf);
    let utf2str = core::str::from_utf8(&read_buf).unwrap();
    println!("file test: read text [{}]", utf2str);

    let fd2: isize = open(0, "hello_world", OpenFlags::O_RDWR, InodeMode::REG);

    let mut read_buf: [u8; 1024] = [0; 1024];
    read(fd2 as usize, &mut read_buf);
    let utf2str = core::str::from_utf8(&read_buf).unwrap();
    println!("file test: read text [{}]", utf2str);

    exit(9)
}
