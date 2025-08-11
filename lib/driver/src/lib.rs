#![no_std]
#![allow(unused)]
#![allow(unknown_lints)]
#![feature(sync_unsafe_cell)]
#![feature(c_variadic)]

use alloc::boxed::Box;
use alloc::sync::Arc;
use async_trait::async_trait;
use core::{
    fmt::{self, Write},
    task::Waker,
};
use device::OSDevice;
use virtio_drivers::transport::{mmio::MmioTransport, pci::PciTransport};

pub use arch::console::console_print;
use arch::console::console_putchar;
use qemu::QUartDevice;
use spin::Once;

pub mod block;
pub mod cpu;
pub mod device;
pub mod hal;
pub mod icu;
pub mod net;
pub mod qemu;
pub mod serial;
pub mod test;

pub use uart_16550::MmioSerialPort;
pub use virtio_drivers::transport::DeviceType;

extern crate alloc;

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
pub static BLOCK_DEVICE2: Once<Arc<dyn BlockDevice>> = Once::new();
pub static CHAR_DEVICE: Once<Arc<dyn CharDevice>> = Once::new();

pub trait BlockDevice: Send + Sync + OSDevice {
    fn read(&self, block_id: usize, buf: &mut [u8]);
    fn write(&self, block_id: usize, buf: &[u8]);
    fn size(&self) -> u64;
    fn block_size(&self) -> usize;
}

#[async_trait]
pub trait CharDevice: Send + Sync + OSDevice {
    fn get(&self, data: &mut u8) -> Result<(), uart_16550::WouldBlockError>;
    fn puts(&self, datas: &[u8]);
    fn handle_irq(&self);

    async fn write(&self, buf: &[u8]) -> usize;
    async fn read(&self, buf: &mut [u8]) -> usize;

    async fn poll_in(&self) -> bool;
    async fn poll_out(&self) -> bool;
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console_print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

macro_rules! wait_for {
    ($cond:expr) => {{
        let mut timeout = 10000000;
        while !$cond && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
    }};
}
pub(crate) use wait_for;
