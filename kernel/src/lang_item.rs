use core::panic::PanicInfo;

use arch::hart::hart_shutdown;
use driver::println;

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("but why panic");
    println!("{:?}", info);

    let sp: usize;
    unsafe {
        core::arch::asm!("mv {}, sp", out(reg) sp);
    }

    println!("stack point: {:#x}", sp);

    hart_shutdown()
}
