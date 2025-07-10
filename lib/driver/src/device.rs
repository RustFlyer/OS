use alloc::{string::String, sync::Arc};
use downcast_rs::DowncastSync;
use virtio_drivers::transport::DeviceType;

use crate::{BlockDevice, CharDevice, net::NetDevice};

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(usize)]
pub enum OSDeviceMajor {
    Serial = 4,
    Block = 8,
    Net = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OSDevId {
    /// Major Device Number
    pub major: OSDeviceMajor,
    /// Minor Device Number. It Identifies different device instances of the
    /// same type
    pub minor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OSDeviceMeta {
    /// Device id.
    pub dev_id: OSDevId,
    /// Name of the device.
    pub name: String,
    /// Mmio start address.
    pub mmio_base: usize,
    /// Mmio size.
    pub mmio_size: usize,
    /// Interrupt number.
    pub irq_no: Option<usize>,
    /// Device type.
    pub dtype: DeviceType,
}

pub trait OSDevice: Sync + Send + DowncastSync {
    fn meta(&self) -> &OSDeviceMeta;

    fn init(&self);

    fn handle_irq(&self);

    fn dev_id(&self) -> OSDevId {
        self.meta().dev_id
    }

    fn name(&self) -> &str {
        &self.meta().name
    }

    fn mmio_base(&self) -> usize {
        self.meta().mmio_base
    }

    fn mmio_size(&self) -> usize {
        self.meta().mmio_size
    }

    fn irq_no(&self) -> Option<usize> {
        self.meta().irq_no
    }

    fn dtype(&self) -> DeviceType {
        self.meta().dtype
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        None
    }

    fn as_char(self: Arc<Self>) -> Option<Arc<dyn CharDevice>> {
        None
    }

    fn as_net(self: Arc<Self>) -> Option<Arc<dyn NetDevice>> {
        None
    }
}
