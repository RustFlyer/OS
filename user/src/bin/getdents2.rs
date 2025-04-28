#![no_std]
#![no_main]

extern crate user_lib;

use config::{
    inode::InodeMode,
    vfs::{AtFlags, OpenFlags},
};
use user_lib::{exit, getdents, open, println};

#[unsafe(no_mangle)]
fn main() {
    let fd = open(
        AtFlags::AT_FDCWD.bits(),
        ".",
        OpenFlags::O_RDONLY,
        InodeMode::DIR,
    );

    let mut buf: [u8; 512] = [0; 512];
    let len = buf.len();

    let _r = getdents(fd as usize, &mut buf, len);

    println!("{:?}", buf);
    let utf2str = core::str::from_utf8(&buf).unwrap();
    println!("{}", utf2str);

    exit(0)
}
