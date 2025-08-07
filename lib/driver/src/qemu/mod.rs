mod uart;
mod virtblk;

use alloc::sync::Arc;
pub use uart::QUartDevice;
pub use virtblk::QVirtBlkDevice;

use crate::{BLOCK_DEVICE, CHAR_DEVICE, println};
