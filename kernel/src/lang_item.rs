use core::panic::PanicInfo;

use arch::hart::hart_shutdown;
use driver::println;

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("{:?}", info);
    hart_shutdown()
}
