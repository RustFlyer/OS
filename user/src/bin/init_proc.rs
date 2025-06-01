#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, waitpid};

#[unsafe(no_mangle)]
fn main() {
    let mut i = 0;

    if fork() == 0 {
        let ret = fork();
        if ret == 0 {
            execve("busybox", &["busybox", "--install", "-s", "/bin"], &[]);
        }

        let mut exitcode = 0;
        waitpid(ret, &mut exitcode);
        execve("shell", &["shell"], &[]);
        // execve(
        //     "busybox",
        //     &["busybox", "sh"],
        //     &[
        //         "PATH=/:/bin",
        //         "LD_LIBRARY_PATH=/:/lib:/lib64",
        //         "TERM=screen",
        //     ],
        // );
    } else {
        loop {
            waitpid(-1, &mut i);
        }
    }
}
