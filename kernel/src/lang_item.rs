use crate::println;
use crate::sbi;
use core::panic::PanicInfo;

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("{:?}", info);
    sbi::shutdown(true);
    loop {}
}
