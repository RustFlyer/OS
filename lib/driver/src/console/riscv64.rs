use crate::CHAR_DEVICE;

pub fn console_putchar(c: u8) {
    #![allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

pub fn console_getchar() -> u8 {
    #![allow(deprecated)]
    sbi_rt::legacy::console_getchar() as u8
}

// Note: This function is not used. What is the purpose of this function?
pub fn getchar() -> u8 {
    let char_device = CHAR_DEVICE.get().unwrap();
    char_device.get()
}
