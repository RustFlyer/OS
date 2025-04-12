#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{console::getchar, execve, print, println, waitpid};

#[unsafe(no_mangle)]
fn main() {
    loop {
        println!("please input app name:");
        let mut bptr = 0;
        let mut buf: [u8; 64] = [0; 64];
        let mut ch = 0;

        while ch != 13 {
            ch = getchar();
            print!("{}", ch as char);
            if ch != 13 {
                buf[bptr] = ch;
                bptr = bptr + 1;
            } else {
                buf[bptr] = 0;
            }
        }

        let apppath = core::str::from_utf8(&buf).unwrap();
        println!("app path is [{}]", apppath);

        execve(apppath, &[], &[]);
    }
}
