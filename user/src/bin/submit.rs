#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, exit, fork, println, sleep, wait, waitpid, yield_};

const TESTCASES: &[&str] = &[
    "basic_testcode.sh",
    "busybox_testcode.sh",
    "libctest_testcode.sh",
    "lua_testcode.sh",
    "netperf_testcode.sh",
    "iozone_testcode.sh",
    // "cyclictest_testcode.sh",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve("busybox", &["busybox", "sh", "-c", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

fn run_test(cmd: &str) {
    if fork() == 0 {
        execve("busybox", &["busybox", "sh", cmd], &[]);
    } else {
        let mut result: i32 = 0;
        waitpid(-1, &mut result);
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    run_cmd("busybox --install -s /bin");
    if fork() == 0 {
        for test in TESTCASES {
            run_test(test);
        }
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid < 0 {
                break;
            }
        }
    }
    0
}
