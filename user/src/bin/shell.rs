#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{console::getchar, execve, exit, fork, print, println, sleep, waitpid};

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
            }
        }

        let buf_slice = &buf[..bptr];
        let apppath = core::str::from_utf8(&buf_slice).unwrap();
        println!("app path is [{}] with len [{}]", apppath, bptr);

        if fork() == 0 {
            execve(apppath, &[], &[]);
            exit(0);
        }
        sleep(3000);
    }
}
