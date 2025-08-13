#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;

use user_lib::{fork, loongarch_init, println, software_init, usershell, waitpid};

#[unsafe(no_mangle)]
fn main() {
    println!("USER SHELL BEGIN!");
    if fork() == 0 {
        println!("FORK OUTPUT!");
        user_lib::test("FORK");
        loongarch_init();
        software_init();
        usershell();
    }
    println!("MAIN OUTPUT!");
    user_lib::test("MAIN");

    let mut dummy = 0;
    loop {
        waitpid(-1, &mut dummy);
    }
}
