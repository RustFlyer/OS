extern crate alloc;

use core::str::from_utf8;

use crate::{
    chdir, close, console::getchar, dup, execve, exit, fork, getcwd, getdents, mkdir, open, print,
    println, waitpid,
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
  [Tab]            Tab key command completion

Type the corresponding command to quickly run the related test.
ATTENTION: you should make sure that relevant file exists in your sdcard!
"#;

pub static mut PWD: String = String::new();

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
        if ch == 9 {
            supple_cmd(buf, &mut tbptr);
            continue;
        }
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
#[allow(static_mut_refs)]
pub fn usershell() {
    let mut buf = [0; 256];
    let mut slice = [0; 256];
    let mut apppath;
    let mut argstring = String::new();
    let mut isinit = false;

    // println!("{}", CMD_HELP);
    CMD_HELP.split('\n').for_each(|s| {
        println!("{}", s);
    });

    loop {
        unsafe { PWD = current_working_path() };
        print!("{}:", unsafe { PWD.clone() });
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
    if open(-100, "/bin/ls", OpenFlags::empty(), InodeMode::empty()) > 0 {
        println!("The device has been initialized");
        return;
    }

    if chdir("musl") < 0 {
        println!("The device uses disk-rv");
        mkdir("/bin");
        run_cmd("./busybox --install -s /bin");
        return;
    }

    mkdir("/bin");
    mkdir("/lib");
    mkdir("/usr");

    close(2);

    run_cmd("./busybox cp /musl/lib/libc.so /lib/ld-musl-riscv64-sf.so.1");
    run_cmd("./busybox cp /musl/lib/libc.so /lib/ld-musl-riscv64.so.1");
    println!("loading user lib: 20%");

    run_cmd("./busybox mv /musl/lib/* /lib/");
    println!("loading user lib: 40%");

    run_cmd("./busybox cp /glibc/lib/libc.so /lib/libc.so.6");
    run_cmd("./busybox cp /glibc/lib/libm.so /lib/libm.so.6");
    println!("loading user lib: 60%");

    run_cmd("./busybox mv /glibc/lib/* /lib/");
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
    if open(-100, "/bin/ls", OpenFlags::empty(), InodeMode::empty()) > 0 {
        println!("The device has been initialized");
        return;
    }

    if chdir("musl") < 0 {
        println!("The device uses disk-la");
        mkdir("/bin");
        run_cmd("./busybox --install -s /bin");
        return;
    }

    mkdir("/bin");
    mkdir("/lib64");
    mkdir("/usr");
    mkdir("/usr/lib64");

    close(2);

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

const BUF_SIZE: usize = 4096;

pub fn list_dir(path: &str) -> Vec<(String, u8)> {
    let mut names = Vec::new();

    let fd = open(
        -100,
        path,
        OpenFlags::O_RDONLY | OpenFlags::O_DIRECTORY,
        InodeMode::empty(),
    );

    if fd < 0 {
        return names;
    }

    let mut buf = [0u8; BUF_SIZE];

    loop {
        let nread = getdents(fd as usize, &mut buf, BUF_SIZE);
        if nread <= 0 {
            break;
        }
        let mut bpos = 0;
        while bpos < nread as usize {
            // resolve linux_dirent64
            let ptr = unsafe { buf.as_ptr().add(bpos) };
            let d_reclen = unsafe { *(ptr.add(16) as *const u16) } as usize;
            let d_type = unsafe { *(ptr.add(18) as *const u8) };
            let name_ptr = unsafe { ptr.add(19) };
            let name_len = (0..(d_reclen - 19))
                .position(|i| unsafe { *name_ptr.add(i) } == 0)
                .unwrap_or(d_reclen - 19);
            let name = unsafe {
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(name_ptr, name_len))
            };
            if name != "." && name != ".." {
                names.push((name.to_string(), d_type));
            }
            bpos += d_reclen;
        }
    }
    close(fd as usize);
    names
}

#[allow(static_mut_refs)]
pub fn supple_cmd(buf: &mut [u8; 256], bptr: &mut usize) {
    let mut tbptr = *bptr;
    let prefix = core::str::from_utf8(&buf[..tbptr]).unwrap_or("");
    let mut files = list_dir(".");

    if tbptr == 0 {
        let matches: Vec<&String> = files
            .iter()
            .map(|(m, _u)| m)
            .filter(|f| f.starts_with(prefix))
            .collect();

        let max_len = matches.iter().map(|s| s.len()).max().unwrap_or(0);

        let last_one = matches.len();
        print!("\n");
        for (i, name) in matches.iter().enumerate() {
            let d_type = files[i].1;
            let padded = format!("{:<width$}", name, width = max_len);
            let colored = match d_type {
                4 => format!("\x1b[1;34m{}\x1b[0m", padded),  // dir blue
                8 => format!("\x1b[1;32m{}\x1b[0m", padded),  // normal green
                10 => format!("\x1b[1;36m{}\x1b[0m", padded), // link light blue
                _ => padded,
            };
            print!("{} ", colored);
            if (i + 1) % 4 == 0 && (i + 1) != last_one {
                print!("\n");
            }
        }
        print!("\n");
        print!("{}:", unsafe { PWD.clone() });
        print!("{}", prefix);
        return;
    }

    files.extend(list_dir("/bin"));
    files.sort();

    let matches: Vec<&String> = files
        .iter()
        .map(|(m, _u)| m)
        .filter(|f| f.starts_with(prefix))
        .collect();

    if matches.len() == 1 {
        let matched = matches[0];
        print!("\r\x1b[2K");
        print!("{}:", unsafe { PWD.clone() });
        for (i, b) in matched.bytes().enumerate() {
            buf[i] = b;
            print!("{}", b as char);
        }
        tbptr = matched.len();
    } else if matches.len() > 1 {
        let max_len = matches.iter().map(|s| s.len()).max().unwrap_or(0);

        let last_one = matches.len();
        print!("\n");
        for (i, name) in matches.iter().enumerate() {
            let d_type = files[i].1;
            let padded = format!("{:<width$}", name, width = max_len);
            let colored = match d_type {
                4 => format!("\x1b[1;34m{}\x1b[0m", padded),  // dir blue
                8 => format!("\x1b[1;32m{}\x1b[0m", padded),  // normal green
                10 => format!("\x1b[1;36m{}\x1b[0m", padded), // link light blue
                _ => padded,
            };
            print!("{} ", colored);
            if (i + 1) % 4 == 0 && (i + 1) != last_one {
                print!("\n");
            }
        }
        print!("\n");
        print!("{}:", unsafe { PWD.clone() });
        print!("{}", prefix);
    }
    *bptr = tbptr;
}
