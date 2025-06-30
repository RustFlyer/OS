#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use config::{inode::InodeMode, vfs::OpenFlags};
use user_lib::{
    chdir, close, console::getchar, dup, execve, exit, fork, mkdir, open, print, println, waitpid,
};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

pub static CMD_HELP: &str = r#"NighthawkOS Quick Command Guide:
  ls /bin          Look up basic commands

  a  or basic      Run all basic tests (busybox sh run-all.sh)
  b  or busybox0   Run busybox test (busybox sh busybox_testcode.sh)
  c  or lua        Run Lua test (busybox sh lua_testcode.sh)
  d  or iozone     Run iozone test (busybox sh iozone_testcode.sh)
  e  or libctest   Run libc test (busybox sh libctest_testcode.sh)
  f  or unixbench  Run UnixBench test (busybox sh unixbench_testcode.sh)
  g  or iperf      Run iperf network test (busybox sh iperf_testcode.sh)
  h  or netperf    Run netperf network test (busybox sh netperf_testcode.sh)

  r7 <arg>         Run static program test (runtest.exe -w entry-static.exe <arg>)
  rd <arg>         Run dynamic program test (runtest.exe -w entry-dynamic.exe <arg>)
  ltp <case>       Run LTP single test case (ltp/testcases/bin/<case>)

Type the corresponding command to quickly run the related test.
ATTENTION: you should make sure that relevant file exists in your sdcard!
"#;

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
        sf if sf.starts_with("ltp ") => {
            let args: Vec<&str> = sf.split(" ").collect();
            let arg = args.get(1).unwrap();
            format!("ltp/testcases/bin/{}", arg).to_string()
        }
        _ => s,
    }
}

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve("./busybox", &["./busybox", "sh", "-c", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

fn parse_args(argstring: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut chars = argstring.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else if quote_char == c {
                    in_quotes = false;
                } else {
                    current.push(c);
                }
            }
            '\\' => {
                if let Some(&next_c) = chars.peek() {
                    if in_quotes && next_c == quote_char {
                        current.push(next_c);
                        chars.next();
                    } else if next_c == '\\' {
                        current.push('\\');
                        chars.next();
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[unsafe(no_mangle)]
fn main() {
    let mut i = 0;

    if fork() == 0 {
        mkdir("/bin");
        mkdir("/lib");

        close(2);

        chdir("musl");
        run_cmd("./busybox cp /musl/lib/* /lib/");
        println!("loading user lib: 20%");

        run_cmd("./busybox cp /musl/lib/libc.so /lib/ld-musl-riscv64-sf.so.1");
        run_cmd("./busybox cp /musl/lib/libc.so /lib/ld-musl-riscv64.so.1");
        println!("loading user lib: 40%");

        run_cmd("./busybox cp /glibc/lib/* /lib/");
        run_cmd("./busybox cp /glibc/lib/libc.so /lib/libc.so.6");
        println!("loading user lib: 60%");

        run_cmd("./busybox cp /glibc/lib/libm.so /lib/libm.so.6");
        run_cmd("./busybox cp /glibc/busybox /bin/");
        println!("loading user lib: 80%");

        run_cmd("./busybox cp /glibc/busybox /");
        run_cmd("./busybox --install -s /bin");
        println!("loading user lib: 100%");
        println!("loading user lib: complete!");
        println!("{}", CMD_HELP);

        let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
        if fd != 2 {
            dup(fd as usize);
            close(fd as usize);
        }

        loop {
            println!("please input app name:");
            let mut bptr: usize = 0;
            let mut buf = [0; 256];
            let mut ch = 0;

            while ch != 13 {
                ch = getchar();
                print!("{}", ch as char);
                if ch != 13 && ch != 127 && bptr < 128 {
                    buf[bptr] = ch;
                    bptr = bptr + 1;
                }
                if bptr > 0 && ch == 127 {
                    bptr = bptr - 1;
                }
            }

            if bptr == 0 {
                continue;
            }

            let buf_slice = &buf[..bptr];
            let apppath = core::str::from_utf8(&buf_slice).unwrap();
            let argstring = apppath.to_string();
            let argstring = easy_cmd(argstring);
            let args: Vec<String> = parse_args(&argstring);

            println!("{}", argstring);

            let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

            if apppath.starts_with("glibc") {
                if chdir("/glibc") < 0 {
                    println!("fail to chdir glibc");
                } else {
                    println!("chdir glibc success");
                }
                continue;
            } else if apppath.starts_with("musl") {
                if chdir("/musl") < 0 {
                    println!("fail to chdir musl");
                } else {
                    println!("chdir musl success");
                }
                continue;
            }

            let mut exitcode = 0;
            let pid = fork();
            if pid == 0 {
                execve(args[0], &args[0..], &[]);
                let bin = format!("/bin/{}", args[0]);
                execve(&bin, &args[0..], &[]);
                println!("{}: not found", args[0]);

                exit(0);
            }
            waitpid(pid, &mut exitcode);
        }
    } else {
        loop {
            waitpid(-1, &mut i);
        }
    }
}
