use crate::CharDevice;
use config::device::MMIO_SERIAL_PORT_ADDR;
use mutex::SpinNoIrqLock;
pub use uart_16550::MmioSerialPort;

pub struct UartDevice {
    device: SpinNoIrqLock<MmioSerialPort>,
}

impl UartDevice {
    pub fn new() -> Self {
        let serialport = unsafe { MmioSerialPort::new(MMIO_SERIAL_PORT_ADDR) };
        Self {
            device: SpinNoIrqLock::new(serialport),
        }
    }

    pub fn new_from(serialport: MmioSerialPort) -> Self {
        Self {
            device: SpinNoIrqLock::new(serialport),
        }
    }
}

impl CharDevice for UartDevice {
    /// Get a Char as u8
    fn get(&self) -> u8 {
        self.device.lock().receive()
    }

    /// Put Chars Out
    ///
    /// - [datas] is buffer for chars
    fn puts(&self, datas: &[u8]) {
        for data in datas {
            self.device.lock().send(*data);
        }
    }

    fn read(&self, buf: &mut [u8]) -> usize {
        let rlen = buf.len();
        let mut r = 0;
        while r < rlen {
            buf[r] = self.device.lock().receive();
            r += 1;
        }
        r
    }

    fn write(&self, buf: &[u8]) -> usize {
        let mut r = 0;
        for data in buf {
            self.device.lock().send(*data);
            r += 1;
        }
        r
    }

    fn handle_irq(&self) {
        todo!()
    }
}
