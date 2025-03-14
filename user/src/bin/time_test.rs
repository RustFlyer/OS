#![no_std]
#![no_main]

// use time::timeval::TimeVal;
use user_lib::{gettimeofday, println};

// extern crate user_lib;

// extern crate alloc;

// #[panic_handler]
// fn user_panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
//     let err = panic_info.message();
//     if let Some(location) = panic_info.location() {
//         println!(
//             "Panicked at {}:{}, {}",
//             location.file(),
//             location.line(),
//             err
//         );
//     } else {
//         println!("Panicked: {}", err);
//     }
//     loop {}
// }

#[unsafe(no_mangle)]
fn main() -> i32 {
    // println!("begin time test");
    // let mut timeval = TimeVal::default();
    // gettimeofday(&mut timeval);
    // println!("timeval: {:?}", timeval);
    0
}
