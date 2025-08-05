extern crate alloc;

use core::str::from_utf8;

use crate::{
    chdir, close, console::getchar, dup, execve, exit, fork, getcwd, getdents, ltpauto, mkdir,
    open, print, println, waitpid,
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
        sf if sf.starts_with("ltprun") => {
            ltpauto::ltprun::autorun();
            format!(" ").to_string()
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
    close(2);

    if chdir("musl") < 0 {
        println!("The device uses disk-rv");
        mkdir("/bin");
        run_cmd("./busybox --install -s /bin");
        let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
        if fd != 2 {
            dup(fd as usize);
            close(fd as usize);
        }
        return;
    }

    if open(-100, "/bin/ls", OpenFlags::empty(), InodeMode::empty()) > 0 {
        println!("The device has been initialized");
        let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
        if fd != 2 {
            dup(fd as usize);
            close(fd as usize);
        }
        return;
    }

    mkdir("/bin");
    mkdir("/lib");
    mkdir("/usr");

    run_cmd("./busybox ln -s /musl/lib/libc.so /lib/ld-musl-riscv64-sf.so.1");
    run_cmd("./busybox ln -s /musl/lib/libc.so /lib/ld-musl-riscv64.so.1");
    println!("loading user lib: 20%");

    run_cmd("./busybox ln -s /musl/lib/* /lib/");
    println!("loading user lib: 40%");

    run_cmd("./busybox ln -s /glibc/lib/libc.so /lib/libc.so.6");
    run_cmd("./busybox ln -s /glibc/lib/libm.so /lib/libm.so.6");
    println!("loading user lib: 60%");

    run_cmd("./busybox ln -s /glibc/lib/* /lib/");
    run_cmd("./busybox ln -s /glibc/busybox /bin/");
    println!("loading user lib: 80%");

    run_cmd("./busybox ln -s /glibc/busybox /");
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
    close(2);

    if chdir("musl") < 0 {
        println!("The device uses disk-la");
        mkdir("/bin");
        run_cmd("./busybox --install -s /bin");
        let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
        if fd != 2 {
            dup(fd as usize);
            close(fd as usize);
        }
        return;
    }

    if open(-100, "/bin/ls", OpenFlags::empty(), InodeMode::empty()) > 0 {
        println!("The device has been initialized");
        let fd = open(0, "/dev/tty", OpenFlags::O_WRONLY, InodeMode::CHAR);
        if fd != 2 {
            dup(fd as usize);
            close(fd as usize);
        }
        return;
    }

    mkdir("/bin");
    mkdir("/lib64");
    mkdir("/usr");
    mkdir("/usr/lib64");

    run_cmd("./busybox ln -s /musl/lib/* /lib64/");
    println!("loading user lib: 20%");

    run_cmd("./busybox ln -s /musl/lib/libc.so /lib64/ld-musl-loongarch-lp64d.so.1");
    run_cmd("./busybox ln -s /glibc/lib/* /lib64/");
    println!("loading user lib: 40%");

    run_cmd("./busybox ln -s /glibc/lib/* /usr/lib64/");
    println!("loading user lib: 60%");

    run_cmd("./busybox ln -s /glibc/busybox /bin/");
    println!("loading user lib: 80%");

    run_cmd("./busybox ln -s /glibc/busybox /");
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

pub fn software_init() {
    gcc_init();
    git_init();
    run_cmd("./busybox ln -s /vim/vim /bin/vim");
}

pub fn gcc_init() {
    #[cfg(target_arch = "riscv64")]
    {
        run_cmd("./busybox ln -s /lib/ld-musl-riscv64.so.1 /lib/libc.musl-riscv64.so.1");
        run_cmd("./busybox ln -s /lib/ld-musl-riscv64.so.1 /lib/libc.so");
    }
    #[cfg(target_arch = "loongarch64")]
    {
        run_cmd("./busybox ln -s /lib/ld-musl-loongarch64.so.1 /lib/libc.musl-loongarch64.so.1");
        run_cmd("./busybox ln -s /lib/ld-musl-loongarch64.so.1 /lib/libc.so");
    }

    run_cmd("./busybox ln -s /lib/libcc1.so.0.0.0 /lib/libcc1.so");
    run_cmd("./busybox ln -s /lib/libmagic.so.1.0.0 /lib/libmagic.so.1");
    run_cmd("./busybox ln -s /lib/libctf-nobfd.so.0.0.0 /lib/libctf-nobfd.so.0");
    run_cmd("./busybox ln -s /lib/libcp1plugin.so.0.0.0 /lib/libcp1plugin.so.0");
    run_cmd("./busybox ln -s /lib/libgomp.so.1.0.0 /lib/libgomp.so.1");
    run_cmd("./busybox ln -s /lib/libcc1plugin.so.0.0.0 /lib/libcc1plugin.so");
    run_cmd("./busybox ln -s /lib/libjansson.so.4.14.1 /lib/libjansson.so.4");
    run_cmd("./busybox ln -s /lib/liblto_plugin.so /lib/liblto_plugin.so");
    run_cmd("./busybox ln -s /lib/libz.so.1.3.1 /lib/libz.so.1");
    run_cmd("./busybox ln -s /lib/libmpfr.so.6.2.1 /lib/libmpfr.so.6");
    run_cmd("./busybox ln -s /lib/libgmp.so.10.5.0 /lib/libgmp.so.10");
    run_cmd("./busybox ln -s /lib/libmpc.so.3.3.1 /lib/libmpc.so.3");
    run_cmd("./busybox ln -s /lib/libctf.so.0.0.0 /lib/libctf.so.0");
    run_cmd("./busybox ln -s /lib/libisl.so.23.3.0 /lib/libisl.so.23");
    run_cmd("./busybox ln -s /lib/libstdc++.so.6.0.33 /lib/libstdc++.so.6");
    run_cmd("./busybox ln -s /lib/libsframe.so.1.0.0 /lib/libsframe.so.1");
    run_cmd("./busybox ln -s /lib/libatomic.so.1.2.0 /lib/libatomic.so.1");
    run_cmd("./busybox ln -s /lib/libcc1plugin.so.0.0.0 /lib/libcc1plugin.so.0");
    run_cmd("./busybox ln -s /lib/libgomp.so.1.0.0 /lib/libgomp.so.1");
    run_cmd("./busybox ln -s /lib/libzstd.so.1.5.7 /lib/libzstd.so.1");
    run_cmd("./busybox ln -s /lib/libstdc++.so.6.0.33 /lib/libstdc++.so");

    run_cmd("./busybox ln -s /usr/bin/gcc /bin/gcc");
}
pub fn git_init() {
    const GIT_COMMANDS: &[&str] = &[
        "git-add",
        "git-am",
        "git-annotate",
        "git-apply",
        "git-archive",
        "git-backfill",
        "git-bisect",
        "git-blame",
        "git-branch",
        "git-bugreport",
        "git-bundle",
        "git-cat-file",
        "git-check-attr",
        "git-check-ignore",
        "git-check-mailmap",
        "git-checkout",
        "git-checkout-index",
        "git-checkout--worker",
        "git-check-ref-format",
        "git-cherry",
        "git-cherry-pick",
        "git-clean",
        "git-clone",
        "git-column",
        "git-commit",
        "git-commit-graph",
        "git-commit-tree",
        "git-config",
        "git-count-objects",
        "git-credential",
        "git-credential-cache",
        "git-credential-cache--daemon",
        "git-credential-store",
        "git-describe",
        "git-diagnose",
        "git-diff",
        "git-diff-files",
        "git-diff-index",
        "git-diff-pairs",
        "git-difftool",
        "git-diff-tree",
        "git-fast-export",
        "git-fast-import",
        "git-fetch",
        "git-fetch-pack",
        "git-fmt-merge-msg",
        "git-for-each-ref",
        "git-for-each-repo",
        "git-format-patch",
        "git-fsck",
        "git-fsck-objects",
        "git-fsmonitor--daemon",
        "git-gc",
        "git-get-tar-commit-id",
        "git-grep",
        "git-hash-object",
        "git-help",
        "git-hook",
        "git-index-pack",
        "git-init",
        "git-init-db",
        "git-interpret-trailers",
        "git-log",
        "git-ls-files",
        "git-ls-remote",
        "git-ls-tree",
        "git-mailinfo",
        "git-mailsplit",
        "git-maintenance",
        "git-merge",
        "git-merge-base",
        "git-merge-file",
        "git-merge-index",
        "git-merge-ours",
        "git-merge-recursive",
        "git-merge-subtree",
        "git-merge-tree",
        "git-mktag",
        "git-mktree",
        "git-multi-pack-index",
        "git-mv",
        "git-name-rev",
        "git-notes",
        "git-pack-objects",
        "git-pack-redundant",
        "git-pack-refs",
        "git-patch-id",
        "git-prune",
        "git-prune-packed",
        "git-pull",
        "git-push",
        "git-range-diff",
        "git-read-tree",
        "git-rebase",
        "git-receive-pack",
        "git-reflog",
        "git-refs",
        "git-remote",
        "git-remote-ext",
        "git-remote-fd",
        "git-repack",
        "git-replace",
        "git-replay",
        "git-rerere",
        "git-reset",
        "git-restore",
        "git-revert",
        "git-rev-list",
        "git-rev-parse",
        "git-rm",
        "git-send-pack",
        "git-shortlog",
        "git-show",
        "git-show-branch",
        "git-show-index",
        "git-show-ref",
        "git-sparse-checkout",
        "git-stage",
        "git-stash",
        "git-status",
        "git-stripspace",
        "git-submodule--helper",
        "git-switch",
        "git-symbolic-ref",
        "git-tag",
        "git-unpack-file",
        "git-unpack-objects",
        "git-update-index",
        "git-update-ref",
        "git-update-server-info",
        "git-upload-archive",
        "git-upload-pack",
        "git-var",
        "git-verify-commit",
        "git-verify-pack",
        "git-verify-tag",
        "git-version",
        "git-whatchanged",
        "git-worktree",
        "git-write-tree",
    ];

    const GIT_BASE_PATH: &str = "/git/libexec/git-core";

    for cmd in GIT_COMMANDS {
        let command = format!(
            "./busybox ln -s {}/git {}/{}",
            GIT_BASE_PATH, GIT_BASE_PATH, cmd
        );
        run_cmd(&command);
    }

    run_cmd("./busybox ln -s /git/bin/git /bin/git");
}
