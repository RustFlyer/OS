#![no_std]
#![no_main]
extern crate user_lib;

use user_lib::riscv_init;
#[allow(unused_imports)]
use user_lib::{chdir, execve, exit, fork, mkdir, println, setuid, wait, waitpid};

const TESTCASES: &[&str] = &[
    // "basic_testcode.sh",
    // "busybox_testcode.sh",
    // "libctest_testcode.sh",
    // "lua_testcode.sh",
    // "iozone_testcode.sh",
    // "cyclictest_testcode.sh",
    // "libcbench_testcode.sh",
    // "lmbench_testcode.sh",
    // "netperf_testcode.sh",
    "copy-file-range_testcode.sh",
    "interrupts_testcode.sh",
    "splice_testcode.sh",
];

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
    println!("start to scan rv-disk");
    riscv_init();

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
        run_test(test);
    }

    chdir("/musl");
    for test in TESTCASES {
        run_test(test);
    }

    exit(114514);
}
