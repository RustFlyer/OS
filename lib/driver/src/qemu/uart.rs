use crate::CharDevice;
use config::device::MMIO_SERIAL_PORT_ADDR;
use mutex::SpinLock;
use uart_16550::MmioSerialPort;

pub struct UartDevice {
    device: SpinLock<MmioSerialPort>,
}

impl UartDevice {
    pub fn new() -> Self {
        let serialport = unsafe { MmioSerialPort::new(MMIO_SERIAL_PORT_ADDR) };
        Self {
            device: SpinLock::new(serialport),
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

    fn handle_irq(&self) {
        todo!()
    }
}
