#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use user_lib::TimeVal;
use user_lib::{gettimeofday, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("begin time test");
    let mut timeval = MaybeUninit::<TimeVal>::uninit();
    gettimeofday(unsafe { timeval.as_mut_ptr().as_mut().unwrap() });
    println!("timeval: {:?}", unsafe { timeval.assume_init() });
    0
}
