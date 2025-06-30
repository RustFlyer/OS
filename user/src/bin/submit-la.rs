#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{chdir, execve, exit, fork, mkdir, println, setuid, wait, waitpid};

const TESTCASES: &[&str] = &[
    "basic_testcode.sh",
    "busybox_testcode.sh",
    "libctest_testcode.sh",
    "lua_testcode.sh",
    "iozone_testcode.sh",
    // "cyclictest_testcode.sh",
    "libcbench_testcode.sh",
    "lmbench_testcode.sh",
    "netperf_testcode.sh",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve("./busybox", &["./busybox", "sh", "-c", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

fn run_test(cmd: &str) {
    if fork() == 0 {
        execve("./busybox", &["./busybox", "sh", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("start to scan la-disk");
    mkdir("/bin");
    mkdir("/lib64");
    mkdir("/usr");
    mkdir("/usr/lib64");

    // run_cmd("./busybox ln -s /musl/lib/libc.so /lib/ld-linux-riscv64-lp64.so.1 ");
    chdir("/glibc");
    run_cmd("./busybox cp /musl/lib/* /lib64/");
    run_cmd("./busybox cp /musl/lib/libc.so /lib64/ld-musl-loongarch-lp64d.so.1");
    run_cmd("./busybox cp /glibc/lib/* /lib64/");
    run_cmd("./busybox cp /glibc/lib/* /usr/lib64/");
    run_cmd("./busybox cp /glibc/busybox /bin/");
    run_cmd("./busybox cp /glibc/busybox /");
    run_cmd("./busybox --install -s /bin");

    if fork() != 0 {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid < 0 {
                break;
            }
        }
    }

    chdir("/glibc");
    for test in TESTCASES {
        if *test == "libctest_testcode.sh" || *test == "netperf_testcode.sh" {
            continue;
        }
        run_test(test);
    }

    chdir("/musl");
    for test in TESTCASES {
        run_test(test);
    }
    run_test("ltp_testcode.sh");

    exit(114514);
}
