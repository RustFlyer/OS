mod uart;
mod virtblk;

use alloc::sync::Arc;
pub use uart::QUartDevice;
pub use virtblk::QVirtBlkDevice;

use crate::{BLOCK_DEVICE, CHAR_DEVICE, println};

pub fn qemu_drive_init() {
    // block can not init without transport
    init_char_device();
}

fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(QUartDevice::new()));
}
