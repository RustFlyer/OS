//! Adapted from PhoenixOS

pub mod uart8250;

use alloc::{boxed::Box, collections::VecDeque, string::ToString, sync::Arc};
use common::RingBuffer;
use core::{
    cell::UnsafeCell,
    cmp,
    fmt::{self, Debug, Write},
    hint::spin_loop,
    task::Waker,
};
use mutex::SpinNoIrqLock;
use osfuture::{suspend_now, take_waker, yield_now};
use virtio_drivers::transport::DeviceType;

use async_trait::async_trait;
use spin::Once;

use super::CharDevice;
use crate::{
    device::{OSDevId, OSDevice, OSDeviceKind, OSDeviceMajor, OSDeviceMeta},
    println,
    serial::{self, uart8250::Uart},
};

const UART_BUF_LEN: usize = 512;

pub static UART0: Once<Arc<dyn CharDevice>> = Once::new();

pub trait UartDriver: Send + Sync {
    fn init(&mut self);
    fn putc(&mut self, byte: u8);
    fn getc(&mut self) -> u8;
    fn poll_in(&self) -> bool;
    fn poll_out(&self) -> bool;
}

pub struct Serial {
    meta: OSDeviceMeta,
    uart: UnsafeCell<Box<dyn UartDriver>>,
    inner: SpinNoIrqLock<SerialInner>,
}

pub struct SerialInner {
    read_buf: RingBuffer,
    /// Hold wakers of pollin tasks.
    pollin_queue: VecDeque<Waker>,
}

unsafe impl Send for Serial {}
unsafe impl Sync for Serial {}

impl Serial {
    /// Create a new Serial. `driver` refers to `Uart` now.
    pub fn new(
        mmio_base: usize,
        mmio_size: usize,
        irq_no: usize,
        driver: Box<dyn UartDriver>,
    ) -> Self {
        let meta = OSDeviceMeta {
            dev_id: OSDevId {
                major: OSDeviceMajor::Serial,
                minor: 0,
            },
            name: "serial".to_string(),
            mmio_base,
            mmio_size,
            irq_no: Some(irq_no),
            dtype: OSDeviceKind::Uart,
            pci_bar: None,
            pci_bdf: None,
            pci_ids: None,
        };

        Self {
            meta,
            uart: UnsafeCell::new(driver),
            inner: SpinNoIrqLock::new(SerialInner {
                read_buf: RingBuffer::new(UART_BUF_LEN),
                pollin_queue: VecDeque::new(),
            }),
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn uart(&self) -> &mut Box<dyn UartDriver> {
        unsafe { &mut *self.uart.get() }
    }

    fn with_mut_inner<T>(&self, f: impl FnOnce(&mut SerialInner) -> T) -> T {
        f(&mut self.inner.lock())
    }
}

impl fmt::Debug for Serial {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Serial")
    }
}

impl OSDevice for Serial {
    fn meta(&self) -> &OSDeviceMeta {
        &self.meta
    }

    fn init(&self) {
        unsafe { &mut *self.uart.get() }.as_mut().init()
    }

    fn handle_irq(&self) {
        // log::error!("handle serial interrupt");
        let uart = self.uart();
        self.with_mut_inner(|inner| {
            while uart.poll_in() {
                let byte = uart.getc();
                // log::info!(
                //     "Serial interrupt handler got byte: {}, ascii: {byte}",
                //     core::str::from_utf8(&[byte]).unwrap()
                // );
                if inner.read_buf.enqueue(byte).is_none() {
                    break;
                }
            }
            // Round Robin
            while let Some(waiting) = inner.pollin_queue.pop_front() {
                waiting.wake();
            }
        });
    }

    fn as_char(self: Arc<Self>) -> Option<Arc<dyn CharDevice>> {
        Some(self)
    }
}

#[async_trait]
impl CharDevice for Serial {
    async fn read(&self, buf: &mut [u8]) -> usize {
        // println!("try to read");
        while !self.poll_in().await {
            // println!("suspend whe read");
            // log::error!("interrupt open: {}", arch::interrupt::is_interrupt_on());
            // arch::interrupt::enable_external_interrupt();
            suspend_now().await;
            // spin_loop();
        }
        let mut len = 0;
        self.with_mut_inner(|inner| {
            len = inner.read_buf.read(buf);
        });
        let uart = self.uart();
        while uart.poll_in() && len < buf.len() {
            let c = uart.getc();
            buf[len] = c;
            len += 1;
        }
        len
    }

    async fn write(&self, buf: &[u8]) -> usize {
        let uart = self.uart();
        for &c in buf {
            let mut ch = c;
            if ch as char == '\n' {
                uart.putc(ch);
                ch = '\r' as u8;
            }
            uart.putc(ch)
        }
        buf.len()
    }

    async fn poll_in(&self) -> bool {
        let uart = self.uart();
        let waker = take_waker().await;
        self.with_mut_inner(|inner| {
            if uart.poll_in() || !inner.read_buf.is_empty() {
                return true;
            }
            inner.pollin_queue.push_back(waker);
            false
        })
    }

    async fn poll_out(&self) -> bool {
        true
    }

    fn get(&self, data: &mut u8) -> Result<(), uart_16550::WouldBlockError> {
        todo!()
    }

    fn puts(&self, datas: &[u8]) {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}
