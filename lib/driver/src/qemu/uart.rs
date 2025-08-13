use core::error::Error;

use crate::{
    CharDevice,
    device::{OSDevId, OSDevice, OSDeviceKind, OSDeviceMajor, OSDeviceMeta},
};
use alloc::{boxed::Box, string::ToString};
use async_trait::async_trait;
use mutex::SpinNoIrqLock;
use uart_16550::MmioSerialPort;

#[cfg(target_arch = "riscv64")]
use config::device::MMIO_SERIAL_PORT_ADDR;
#[cfg(target_arch = "loongarch64")]
use config::device::PCI_SERIAL_PORT_ADDR;
#[cfg(target_arch = "loongarch64")]
use config::device::UART_ADDR_LA_BOARD;
use virtio_drivers::transport::mmio::MmioError;

pub struct QUartDevice {
    meta: OSDeviceMeta,
    pub device: SpinNoIrqLock<MmioSerialPort>,
}

impl QUartDevice {
    pub fn new(mmio_base: usize, mmio_size: usize, irq_no: usize) -> Self {
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

        #[cfg(target_arch = "loongarch64")]
        let serialport = unsafe { MmioSerialPort::new(UART_ADDR_LA_BOARD) };
        #[cfg(target_arch = "riscv64")]
        let serialport = unsafe { MmioSerialPort::new(MMIO_SERIAL_PORT_ADDR) };

        Self {
            meta,
            device: SpinNoIrqLock::new(serialport),
        }
    }
}

impl OSDevice for QUartDevice {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        &self.meta
    }

    fn init(&self) {}

    fn handle_irq(&self) {}
}

#[async_trait]
impl CharDevice for QUartDevice {
    /// Get a Char as u8
    fn get(&self, data: &mut u8) -> Result<(), uart_16550::WouldBlockError> {
        *data = self.device.lock().try_receive()?;
        Ok(())
    }

    /// Put Chars Out
    fn puts(&self, datas: &[u8]) {
        for data in datas {
            self.device.lock().send(*data);
        }
    }

    async fn read(&self, buf: &mut [u8]) -> usize {
        let rlen = buf.len();
        let mut r = 0;
        while r < rlen {
            buf[r] = self.device.lock().receive();
            r += 1;
        }
        r
    }

    async fn write(&self, buf: &[u8]) -> usize {
        let mut r = 0;
        for data in buf {
            let mut ch = *data;
            if ch as char == '\n' {
                self.device.lock().send(ch);
                ch = '\r' as u8;
            }
            self.device.lock().send(ch);
            r += 1;
        }
        r
    }

    async fn poll_in(&self) -> bool {
        // stupid. When you watch it, the data will be lost
        self.device.lock().try_receive().is_ok()
    }

    async fn poll_out(&self) -> bool {
        true
    }

    fn handle_irq(&self) {
        todo!()
    }
}
