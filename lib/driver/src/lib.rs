#![no_std]
#![no_main]
#![allow(non_upper_case_globals)]

use core::task::Waker;
extern crate alloc;
use alloc::sync::Arc;
use qemu::{UartDevice, VirtBlkDevice};
use spin::Once;

pub mod qemu;
pub mod sbi;

static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
static CHAR_DEVICE: Once<Arc<dyn CharDevice>> = Once::new();

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

pub fn init_block_device() {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtBlkDevice::new()));
}

pub fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new()));
}
