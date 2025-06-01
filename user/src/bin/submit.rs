#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, exit, fork, println, sleep, wait, waitpid, yield_};

const TESTCASES: &[&str] = &[
    "run-all.sh",
    // "busybox_testcode.sh",
    // "lua_testcode.sh",
    // "netperf_testcode.sh",
    // "libctest_testcode.sh",
    // "iozone_testcode.sh",
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
        exit(33024);
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let _pid = wait(&mut exit_code);
            println!("exit: {}", exit_code);
            if exit_code == 33024 {
                println!("break: {}", exit_code);
                break;
            }
        }
    }
    0
}
