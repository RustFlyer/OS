#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use user_lib::{fork, riscv_init, software_init, usershell, waitpid};

#[unsafe(no_mangle)]
fn main() {
    if fork() == 0 {
        riscv_init();
        software_init();
        user_lib::enable_err();
        user_lib::enable_usrlog();
        usershell();
    }

    let mut dummy = 0;
    loop {
        waitpid(-1, &mut dummy, 0);
    }
}
