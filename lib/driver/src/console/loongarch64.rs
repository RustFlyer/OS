use spin::Mutex;

#[cfg(not(board = "2k1000"))]
const UART_ADDR: usize = 0x01FE001E0 | config::mm::VIRT_START;
#[cfg(board = "2k1000")]
const UART_ADDR: usize = 0x800000001fe20000;
// 0x800000001fe20000ULL
static COM1: Mutex<Uart> = Mutex::new(Uart::new(UART_ADDR));

struct Uart {
    base_address: usize,
}

impl Uart {
    const fn new(base_address: usize) -> Self {
        Uart { base_address }
    }

    fn putchar(&mut self, c: u8) {
        let ptr = self.base_address as *mut u8;
        loop {
            unsafe {
                if ptr.add(5).read_volatile() & (1 << 5) != 0 {
                    break;
                }
            }
        }
        unsafe {
            ptr.add(0).write_volatile(c);
        }
    }

    fn getchar(&mut self) -> Option<u8> {
        let ptr = self.base_address as *mut u8;
        unsafe {
            if ptr.add(5).read_volatile() & 1 == 0 {
                // The DR bit is 0, meaning no data
                None
            } else {
                // The DR bit is 1, meaning data!
                Some(ptr.add(0).read_volatile())
            }
        }
    }
}

pub fn console_putchar(c: usize) -> usize {
    if ch == b'\n' {
        COM1.lock().putchar(b'\r');
    }
    COM1.lock().putchar(ch)
}

pub fn console_getchar() -> u8 {
    COM1.lock().getchar()
}
