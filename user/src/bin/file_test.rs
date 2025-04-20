#![no_std]
#![no_main]

extern crate user_lib;

use core::{
    ffi::CStr,
    ptr::{self, null},
};

use config::{
    inode::InodeMode,
    vfs::{AtFd, AtFlags, OpenFlags},
};
use user_lib::{execve, exit, fork, lseek, open, println, read, sleep, write, yield_};

#[unsafe(no_mangle)]
fn main() {
    {
        let fd: isize = open(
            AtFlags::AT_FDCWD.bits(),
            "tes",
            OpenFlags::O_CREAT | OpenFlags::O_RDWR,
            InodeMode::REG,
        );
        println!("file test: fd of tes is [{}]", fd);

        let write_text = "Moon rises at night.".repeat(16);

        println!("file test: try to write [{}]", write_text);
        write(fd as usize, write_text.as_bytes());
        println!("file test: finish write");

        lseek(fd as usize, 0, 0);
        println!("file test: lseek to origin");

        let mut read_buf: [u8; 1024] = [0; 1024];
        read(fd as usize, &mut read_buf);
        let utf2str = core::str::from_utf8(&read_buf).unwrap();
        println!("file test: read text [{}]", utf2str);
    }

    {
        let fd2: isize = open(
            AtFlags::AT_FDCWD.bits(),
            "add",
            OpenFlags::O_RDWR,
            InodeMode::REG,
        );
        let mut read_buf: [u8; 1024] = [0; 1024];
        read(fd2 as usize, &mut read_buf);
        // println!("file test: read tbuf [{:?}]", read_buf);
        // let utf2str = core::str::from_utf8(&read_buf).unwrap();
        // println!("file test: read text [{}]", utf2str);
        // lseek(fd2 as usize, 0, 0);
        println!("execve-test begin to run");
        let argvs = ["Sun", "Star"];
        let envps = ["Loop", "Func"];
        execve("add", &argvs, &envps);
    }

    exit(9)
}
