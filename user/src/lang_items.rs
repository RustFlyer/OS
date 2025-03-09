use core::panic::PanicInfo;

fn user_panic_handler(panic_info: &PanicInfo) -> ! {
    let err = panic_info.message();
    if let Some(location) = panic_info.location() {
        println!(
            "Panicked at {}:{}, {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        println!("Panicked: {}", err);
    }
    loop {}
}

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    user_panic_handler(panic_info);
}
