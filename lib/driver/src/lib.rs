#![no_std]
#![allow(unused)]
#![allow(unknown_lints)]

use alloc::sync::Arc;
use core::{
    fmt::{self, Write},
    task::Waker,
};
use virtio_drivers::transport::{mmio::MmioTransport, pci::PciTransport};

use console::console_putchar;
use qemu::UartDevice;
use spin::Once;

pub mod block;
pub mod cpu;
pub mod device;
pub mod hal;
pub mod net;
pub mod plic;
pub mod qemu;

pub use uart_16550::MmioSerialPort;
pub use virtio_drivers::transport::DeviceType;

pub mod console;

extern crate alloc;

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
pub static BLOCK_DEVICE2: Once<Arc<dyn BlockDevice>> = Once::new();
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
    // init_block_device();
    init_char_device();

    // let buf = "hello char dev\n";
    // CHAR_DEVICE.get().unwrap().write(buf.as_bytes());
    println!("[CHAR_DEVICE] INIT SUCCESS");
}

fn init_block_device() {
    log::debug!("block in");
    // BLOCK_DEVICE.call_once(|| Arc::new(VirtBlkDevice::new()));
    log::debug!("block out");
}

fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new()));
}

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            console_putchar(*c);
        }
        Ok(())
    }
}

pub fn console_print(args: fmt::Arguments<'_>) {
    // Note: Is the lock necessary?
    // static PRINT_MUTEX: SpinNoIrqLock<()> = SpinNoIrqLock::new(());
    // let _lock = PRINT_MUTEX.lock();
    Console.write_fmt(args).unwrap();
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
            // core::hint::spin_loop();
            timeout -= 1;
        }
    }};
}
pub(crate) use wait_for;
