#![no_std]

use alloc::sync::Arc;
use core::task::Waker;

use qemu::{UartDevice, VirtBlkDevice};
use spin::Once;

pub mod qemu;
pub mod sbi;

extern crate alloc;

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
pub static CHAR_DEVICE: Once<Arc<dyn CharDevice>> = Once::new();

pub trait BlockDevice: Send + Sync {
    fn read(&self, block_id: usize, buf: &mut [u8]);
    fn write(&self, block_id: usize, buf: &[u8]);
    fn size(&self) -> u64;
    fn block_size(&self) -> usize;
}

pub trait CharDevice: Send + Sync {
    fn get(&self) -> u8;
    fn puts(&self, datas: &[u8]);
    fn handle_irq(&self);

    fn write(&self, buf: &[u8]) -> usize;
    fn read(&self, buf: &mut [u8]) -> usize;

    fn waker(&self, _waker: Waker) {
        todo!()
    }
}

pub fn init() {
    init_block_device();
    init_char_device();
}

fn init_block_device() {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtBlkDevice::new()));
}

fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new()));
}

pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.get().unwrap();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 100..553 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write(i as usize, &write_buffer);
        block_device.read(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::sbi::console_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::print!($($arg)*);
        $crate::print!("\n")
    }};
}
