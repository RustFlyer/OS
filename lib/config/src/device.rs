use super::mm::KERNEL_MAP_OFFSET;

pub const MAX_HARTS: usize = 4;

pub const MMIO_SERIAL_PORT_ADDR: usize = 0x1000000 + KERNEL_MAP_OFFSET;

pub const VIRTIO0: usize = 0x10001000 + KERNEL_MAP_OFFSET;

pub const BLOCK_SIZE: usize = 4096;
