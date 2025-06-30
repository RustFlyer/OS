#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use user_lib::{console::getchar, execve, exit, fork, print, println, waitpid};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

pub fn easy_cmd(s: String) -> String {
    let str = s.as_str();
    match str {
        "a" | "basic" => "busybox sh run-all.sh".to_string(),
        "b" | "busybox0" => "busybox sh busybox_testcode.sh".to_string(),
        "c" | "lua" => "busybox sh lua_testcode.sh".to_string(),
        "d" | "iozone" => "busybox sh iozone_testcode.sh".to_string(),
        "e" | "libctest" => "busybox sh libctest_testcode.sh".to_string(),
        "f" | "unixbench" => "busybox sh unixbench_testcode.sh".to_string(),
        "g" | "iperf" => "busybox sh iperf_testcode.sh".to_string(),
        "h" | "netperf" => "busybox sh netperf_testcode.sh".to_string(),
        sf if sf.starts_with("r7") => {
            let args: Vec<&str> = sf.split(" ").collect();
            let arg = args.get(1).unwrap();
            format!("runtest.exe -w entry-static.exe {}", arg).to_string()
        }
        sf if sf.starts_with("rd") => {
            let args: Vec<&str> = sf.split(" ").collect();
            let arg = args.get(1).unwrap();
            format!("runtest.exe -w entry-dynamic.exe {}", arg).to_string()
        }
        _ => s,
    }
}

#[unsafe(no_mangle)]
fn main() {
    loop {
        println!("please input app name:");
        let mut bptr = 0;
        let mut buf = [0; 128];
        let mut ch = 0;

        while ch != 13 {
            ch = getchar();
            print!("{}", ch as char);
            if ch != 13 && ch != 127 {
                buf[bptr] = ch;
                bptr = bptr + 1;
            }
            if ch == 127 {
                bptr = bptr - 1;
            }
        }

        let buf_slice = &buf[..bptr];
        let apppath = core::str::from_utf8(&buf_slice).unwrap();
        let argstring = apppath.to_string();

        let argstring = easy_cmd(argstring);

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
