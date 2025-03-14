#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::*;

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

#[allow(unused)]
fn sleepy() {
    // let time: usize = 1000;
    // let mut rem = TimeSpec::from_ms(0);
    // for i in 1..=5 {
    //     sleep(time);
    //     println!("sleep {} x {} msecs.", i, time);
    // }
    exit(0);
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    // println!("begin sleep test");
    // let mut old_time_val = TimeVal::from_usec(0);
    // gettimeofday(&mut old_time_val);
    // let pid = fork();
    // let mut exit_code: i32 = 0;
    // if pid == 0 {
    //     println!("Child process begins sleepy");
    //     sleepy();
    // }
    // assert!(waitpid(pid as usize, &mut exit_code) == pid && exit_code == 0);
    // let mut new_time_val = TimeVal::from_usec(0);
    // gettimeofday(&mut new_time_val);
    // println!(
    //     "use {} usecs.",
    //     new_time_val.into_usec() - old_time_val.into_usec()
    // );
    // println!("sleep pass.");
    0
}
