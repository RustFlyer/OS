pub fn console_putchar(c: u8) {
    #![allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

pub fn console_getchar() -> u8 {
    #![allow(deprecated)]
    sbi_rt::legacy::console_getchar() as u8
}
