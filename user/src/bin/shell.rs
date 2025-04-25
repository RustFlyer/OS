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
        let raws: Vec<String> = argstring.split(' ').map(|s| s.to_string()).collect();
        let mut args: Vec<String> = Vec::new();

        let mut tmp: String = String::new();
        let mut is_close = 0;

        for raw in raws {
            let mut raw = raw.clone();
            if raw.starts_with('"') {
                is_close += 1;
                raw.remove(0);
            }
            if raw.ends_with('"') {
                is_close -= 1;
                raw.remove(raw.len() - 1);
            }
            tmp = tmp + &raw;
            if is_close == 0 {
                args.push(tmp.clone());
                tmp.clear();
            } else {
                tmp = tmp + " ";
            }
        }

        println!("app path is [{}] with len [{}]", apppath, bptr);

        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let mut exitcode = 0;
        let pid = fork();
        if pid == 0 {
            execve(args[0], &args[0..], &[]);
            exit(0);
        }
        waitpid(pid, &mut exitcode);
    }
}
