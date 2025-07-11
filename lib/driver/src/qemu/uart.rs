use crate::{CharDevice, device::OSDevice};
use alloc::boxed::Box;
use async_trait::async_trait;
use mutex::SpinNoIrqLock;
use uart_16550::MmioSerialPort;

#[cfg(target_arch = "riscv64")]
use config::device::MMIO_SERIAL_PORT_ADDR;
#[cfg(target_arch = "loongarch64")]
use config::device::PCI_SERIAL_PORT_ADDR;

pub struct QUartDevice {
    pub device: SpinNoIrqLock<MmioSerialPort>,
}

impl QUartDevice {
    pub fn new() -> Self {
        #[cfg(target_arch = "loongarch64")]
        let serialport = unsafe { MmioSerialPort::new(PCI_SERIAL_PORT_ADDR) };
        #[cfg(target_arch = "riscv64")]
        let serialport = unsafe { MmioSerialPort::new(MMIO_SERIAL_PORT_ADDR) };
        Self {
            device: SpinNoIrqLock::new(serialport),
        }
    }

    pub fn new_from_mmio(serialport: MmioSerialPort) -> Self {
        Self {
            device: SpinNoIrqLock::new(serialport),
        }
    }
}

impl Default for QUartDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl OSDevice for QUartDevice {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        todo!()
    }

    fn init(&self) {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}

#[async_trait]
impl CharDevice for QUartDevice {
    /// Get a Char as u8
    fn get(&self) -> u8 {
        self.device.lock().receive()
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
            self.device.lock().send(*data);
            r += 1;
        }
        r
    }

    async fn poll_in(&self) -> bool {
        true
    }

    async fn poll_out(&self) -> bool {
        true
    }

    fn handle_irq(&self) {
        todo!()
    }
}
