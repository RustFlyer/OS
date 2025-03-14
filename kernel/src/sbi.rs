use sbi_rt::{system_reset, NoReason, Shutdown, SystemFailure};

#[allow(unused)]
pub fn console_putchar(c: usize) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}

#[allow(unused)]
pub fn console_getchar() -> usize {
    #[allow(deprecated)]
    sbi_rt::legacy::console_getchar()
}

#[allow(unused)]
pub fn set_timer(us: usize) {
    sbi_rt::set_timer(us.try_into().unwrap());
}

pub fn shutdown(failure: bool) {
    if !failure {
        system_reset(Shutdown, NoReason);
    } else {
        system_reset(Shutdown, SystemFailure);
    }
}
