extern crate alloc;

use core::str::from_utf8;

use crate::{
    chdir, close, console::getchar, dup, execve, exit, fork, getcwd, mkdir, open, print, println,
    waitpid,
};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use config::{inode::InodeMode, vfs::OpenFlags};

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

  [Enter]          Run the last command

Type the corresponding command to quickly run the related test.
ATTENTION: you should make sure that relevant file exists in your sdcard!
"#;

pub fn easy_cmd(s: String) -> String {
    let str = s.as_str();
    match str {
        "a" | "basic" => "busybox sh basic_testcode.sh".to_string(),
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

pub fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve("./busybox", &["./busybox", "sh", "-c", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

pub fn parse_args(argstring: &str) -> Vec<String> {
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

pub fn typecmd(buf: &mut [u8; 256], bptr: &mut usize) {
    let mut ch = 0;
    let mut tbptr = *bptr;
    while ch != '\n' as u8 {
        ch = getchar();
        if ch != 13 && ch != 127 && ch != '\n' as u8 && tbptr < 128 {
            buf[tbptr] = ch;
            tbptr = tbptr + 1;
        }
        if tbptr > 0 && ch == 127 {
            tbptr = tbptr - 1;
        }
    }
    *bptr = tbptr;
}

pub fn ischangedir(args: Vec<&str>) -> bool {
    if args.len() < 2 {
        return false;
    }

    let is_chdir_inst = args[0] == "cd";
    let chdir_path = args[1];

    if !is_chdir_inst {
        return false;
    }

    if chdir(chdir_path) >= 0 {
        println!("change path to: {}", chdir_path);
        return true;
    }

    println!("fail to chdir");
    return is_chdir_inst;
}

pub fn current_working_path() -> String {
    let mut buf: [u8; 64] = [0; 64];
    let _res = getcwd(buf.as_mut_ptr(), buf.len());

    from_utf8(&buf).unwrap().to_string()
}

#[inline]
pub fn usershell() {
    let mut buf = [0; 256];
    let mut slice = [0; 256];
    let mut apppath;
    let mut argstring = String::new();
    let mut isinit = false;

    println!("{}", CMD_HELP);

    loop {
        print!("{}:", current_working_path());
        let mut bptr = 0;
        typecmd(&mut buf, &mut bptr);

        if bptr != 0 {
            isinit = true;
            slice.copy_from_slice(&buf);
            apppath = core::str::from_utf8(&slice[..bptr]).unwrap();
            argstring = apppath.to_string();
            argstring = easy_cmd(argstring);
        }

        if !isinit {
            continue;
        }

        let _args: Vec<String> = parse_args(&argstring);
        let args: Vec<&str> = _args.iter().map(|s| s.as_str()).collect();

        if ischangedir(args.clone()) {
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
}

pub fn riscv_init() {
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

    let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
    if fd != 2 {
        dup(fd as usize);
        close(fd as usize);
    }
}

pub fn loongarch_init() {
    mkdir("/bin");
    mkdir("/lib64");
    mkdir("/usr");
    mkdir("/usr/lib64");

    close(2);

    chdir("/musl");
    run_cmd("./busybox cp /musl/lib/* /lib64/");
    println!("loading user lib: 20%");

    run_cmd("./busybox cp /musl/lib/libc.so /lib64/ld-musl-loongarch-lp64d.so.1");
    run_cmd("./busybox cp /glibc/lib/* /lib64/");
    println!("loading user lib: 40%");

    run_cmd("./busybox cp /glibc/lib/* /usr/lib64/");
    println!("loading user lib: 60%");

    run_cmd("./busybox cp /glibc/busybox /bin/");
    println!("loading user lib: 80%");

    run_cmd("./busybox cp /glibc/busybox /");
    run_cmd("./busybox --install -s /bin");
    println!("loading user lib: 100%");
    println!("loading user lib: complete!");

    let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
    if fd != 2 {
        dup(fd as usize);
        close(fd as usize);
    }
}
