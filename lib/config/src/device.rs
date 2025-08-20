use super::mm::KERNEL_MAP_OFFSET;

pub const MAX_HARTS: usize = 5;

pub const MMIO_SERIAL_PORT_ADDR: usize = 0x10000000 + KERNEL_MAP_OFFSET;

pub const PCI_SERIAL_PORT_ADDR: usize = 0x1fe001e0 + KERNEL_MAP_OFFSET;

// on board
// pub const UART_ADDR_LA_BOARD: usize = 0x8000_0000_1fe2_0000;
pub const DEVICE_MAP_LA: usize = 0x8000_0000_0000_0000;

// on qemu
pub const UART_ADDR_LA_BOARD: usize = PCI_SERIAL_PORT_ADDR;

pub const VIRTIO0: usize = 0x10001000 + KERNEL_MAP_OFFSET;
// pub const VIRTIO0: usize = 0x10001000 + KERNEL_MAP_OFFSET;

pub const BLOCK_SIZE: usize = 512;

pub const DEV_SIZE: u64 = 4096 * 1024 * 1024;
