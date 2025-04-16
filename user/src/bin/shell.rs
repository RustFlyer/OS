#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use user_lib::{console::getchar, execve, exit, fork, print, println, sleep, waitpid};

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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
        let argstring = apppath.to_string();
        let args: Vec<&str> = argstring.split(' ').collect();
        println!("app path is [{}] with len [{}]", apppath, bptr);

        let mut exitcode = 0;
        let pid = fork();
        if pid == 0 {
            execve(args[0], &args[1..], &[]);
            exit(0);
        }
        waitpid(pid, &mut exitcode);
    }
}
