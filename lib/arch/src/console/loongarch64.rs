use spin::Mutex;

/// UART address of stdout device for Qemu `virt` machine.
///
/// This is `serial@1fe001e0` in the device tree.
///
/// See https://github.com/LoongsonLab/oscomp-documents/blob/main/platforms.md
///
/// # Note
/// I don't know why the address is in the `0x9xxx_...` range, which has MAT = 1.
// #[allow(unknown_lints)]
// #[cfg(not(board = "2k1000"))]
const UART_ADDR: usize = 0x9000_0000_1fe0_01e0;

/// UART address of stdout device for Loongson 2K1000 machine.
///
/// This is `serial@0x1fe20000` in the device tree.
///
/// See https://github.com/LoongsonLab/oscomp-documents/blob/main/platforms.md
// #[allow(unknown_lints)]
// #[cfg(board = "2k1000")]
// const UART_ADDR: usize = 0x8000_0000_1fe2_0000;

static COM1: Mutex<Uart> = Mutex::new(Uart::new(UART_ADDR));

struct Uart {
    base_address: usize,
}

// TODO: How does `putchar` and `getchar` work?
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

pub fn console_putchar(c: u8) {
    if c == b'\n' {
        COM1.lock().putchar(b'\r');
    }
    COM1.lock().putchar(c)
}

pub fn console_getchar() -> u8 {
    // TODO: Handle the case when no data is available (?)
    COM1.lock().getchar().unwrap_or(0)
}
