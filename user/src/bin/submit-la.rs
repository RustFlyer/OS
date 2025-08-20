#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::loongarch_init;
#[allow(unused_imports)]
use user_lib::{chdir, execve, exit, fork, mkdir, println, setuid, wait, waitpid};
use user_lib::{runltp_lagl, runltp_laml};

const TESTCASES: &[&str] = &[
    "basic_testcode.sh",
    "busybox_testcode.sh",
    "libctest_testcode.sh",
    "lua_testcode.sh",
    // "netperf_testcode.sh",
    // "iperf_testcode.sh",
    // "iozone_testcode.sh",
    // "cyclictest_testcode.sh",
    // "libcbench_testcode.sh",
    // "lmbench_testcode.sh",
];

fn run_test(cmd: &str) {
    if fork() == 0 {
        execve("./busybox", &["./busybox", "sh", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result, 0);
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("start to scan la-disk");
    loongarch_init();

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
    runltp_lagl();

    chdir("/musl");
    runltp_laml();

    chdir("/glibc");
    for test in TESTCASES {
        run_test(test);
    }

    chdir("/musl");
    for test in TESTCASES {
        run_test(test);
    }

    exit(114514);
}
