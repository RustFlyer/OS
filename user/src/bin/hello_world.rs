#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, println};

// #[panic_handler]
// fn user_panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
//     let err = panic_info.message();
//     if let Some(location) = panic_info.location() {
//         println!(
//             "Panicked at {}:{}, {}",
//             location.file(),
//             location.line(),
//             err
//         );`
//     } else {
//         println!("Panicked: {}", err);
//     }
//     loop {}
// }

#[unsafe(no_mangle)]
fn main() {
    println!("test 0??????????");
    println!("hello world");
    exit(3)
}
