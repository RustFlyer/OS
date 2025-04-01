#![no_std]
#![no_main]
#![allow(non_upper_case_globals)]

use core::task::Waker;
extern crate alloc;
use alloc::sync::Arc;
use core::fmt::{self};
use qemu::{UartDevice, VirtBlkDevice};
use spin::Once;
pub mod qemu;
pub mod sbi;

pub use sbi::sbi_print;

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
pub static CHAR_DEVICE: Once<Arc<dyn CharDevice>> = Once::new();

pub trait BlockDevice: Send + Sync {
    fn read(&self, block_id: usize, buf: &mut [u8]);
    fn write(&self, block_id: usize, buf: &[u8]);
    fn size(&self) -> usize;
}

pub trait CharDevice: Send + Sync {
    fn get(&self) -> u8;
    fn puts(&self, datas: &[u8]);
    fn handle_irq(&self);

    fn waker(&self, _waker: Waker) {
        todo!()
    }
}

pub fn init() {
    init_block_device();
    init_char_device();
}

pub fn init_block_device() {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtBlkDevice::new()));
}

pub fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new()));
}

pub fn shutdown(failure: bool) -> ! {
    sbi::hart_shutdown(failure);
}

pub fn set_timer(timer: usize) {
    sbi::set_timer(timer);
}

pub fn print(args: fmt::Arguments<'_>) {
    sbi_print(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::print(format_args_nl!($($arg)*));
    }};
}
