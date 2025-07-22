use crate::BlockDevice;
use crate::device::{OSDevId, OSDevice, OSDeviceKind, OSDeviceMajor, OSDeviceMeta};
use crate::hal::VirtHalImpl;
use alloc::string::ToString;
use alloc::sync::Arc;
use config::device::{BLOCK_SIZE, DEV_SIZE, VIRTIO0};
use mutex::SpinNoIrqLock;
use virtio_drivers::transport::{DeviceType, Transport};
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{mmio::MmioTransport, pci::PciTransport},
};

pub struct VirtBlkDevice<T: Transport> {
    block: SpinNoIrqLock<VirtIOBlk<VirtHalImpl, T>>,
    meta: OSDeviceMeta,
}

impl<T: Transport> VirtBlkDevice<T> {
    pub fn new(
        mmio_base: usize,
        mmio_size: usize,
        irq_no: Option<usize>,
        transport: T,
    ) -> Arc<Self> {
        let meta = OSDeviceMeta {
            dev_id: OSDevId {
                major: OSDeviceMajor::Block,
                minor: 0,
            },
            name: "block".to_string(),
            mmio_base,
            mmio_size,
            irq_no,
            dtype: OSDeviceKind::Virtio(DeviceType::Block),
            pci_bar: None,
            pci_bdf: None,
            pci_ids: None,
        };

        let blk = VirtIOBlk::<VirtHalImpl, T>::new(transport);
        if let Err(e) = blk {
            log::error!("blk: {:?}", e);
        }

        Arc::new(Self {
            block: SpinNoIrqLock::new(blk.unwrap()),
            meta,
        })
    }

    pub fn try_new(
        mmio_base: usize,
        mmio_size: usize,
        irq_no: Option<usize>,
        transport: T,
    ) -> Option<Arc<Self>> {
        let meta = OSDeviceMeta {
            dev_id: OSDevId {
                major: OSDeviceMajor::Block,
                minor: 0,
            },
            name: "block".to_string(),
            mmio_base,
            mmio_size,
            irq_no,
            dtype: OSDeviceKind::Virtio(DeviceType::Block),
            pci_bar: None,
            pci_bdf: None,
            pci_ids: None,
        };

        let blk = VirtIOBlk::<VirtHalImpl, T>::new(transport);
        if let Err(e) = blk {
            log::error!("blk: {:?}", e);
            return None;
        }

        Some(Arc::new(Self {
            block: SpinNoIrqLock::new(blk.unwrap()),
            meta,
        }))
    }
}

unsafe impl Sync for VirtBlkDevice<MmioTransport<'static>> {}
unsafe impl Send for VirtBlkDevice<MmioTransport<'static>> {}
unsafe impl Sync for VirtBlkDevice<PciTransport> {}
unsafe impl Send for VirtBlkDevice<PciTransport> {}

impl BlockDevice for VirtBlkDevice<MmioTransport<'static>> {
    /// Read Block
    ///
    /// - ['block_id'] is the id of block in VirtHW
    /// - ['buf'] is the buffer for datas
    ///
    /// Data from Block to Buf
    fn read(&self, block_id: usize, buf: &mut [u8]) {
        let res = self.block.lock().read_blocks(block_id, buf);
        // log::info!("read block id [{}] buf [{:?}]", block_id, buf);
        if res.is_err() {
            panic!(
                "Error when reading VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
    }

    /// Write Block
    ///
    /// - [block_id] is the id of block in VirtHW
    /// - [buf] is the buffer for datas
    ///
    /// Data from Buf to Block
    fn write(&self, block_id: usize, buf: &[u8]) {
        // log::info!("write tick {} with {:?}", block_id, buf);
        let res = self.block.lock().write_blocks(block_id, buf);
        if res.is_err() {
            panic!(
                "Error when writing VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    /// Get Block Size
    fn size(&self) -> u64 {
        DEV_SIZE
    }
}

impl BlockDevice for VirtBlkDevice<PciTransport> {
    /// Read Block
    ///
    /// - ['block_id'] is the id of block in VirtHW
    /// - ['buf'] is the buffer for datas
    ///
    /// Data from Block to Buf
    fn read(&self, block_id: usize, buf: &mut [u8]) {
        let res = self.block.lock().read_blocks(block_id, buf);
        // log::info!("read block id [{}] buf [{:?}]", block_id, buf);
        if res.is_err() {
            panic!(
                "Error when reading VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
    }

    /// Write Block
    ///
    /// - [block_id] is the id of block in VirtHW
    /// - [buf] is the buffer for datas
    ///
    /// Data from Buf to Block
    fn write(&self, block_id: usize, buf: &[u8]) {
        // log::info!("write tick {} with {:?}", block_id, buf);
        let res = self.block.lock().write_blocks(block_id, buf);
        if res.is_err() {
            panic!(
                "Error when writing VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    /// Get Block Size
    fn size(&self) -> u64 {
        DEV_SIZE
    }
}

impl OSDevice for VirtBlkDevice<MmioTransport<'static>> {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        &self.meta
    }

    fn init(&self) {}

    fn handle_irq(&self) {}
}

impl OSDevice for VirtBlkDevice<PciTransport> {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        &self.meta
    }

    fn init(&self) {}

    fn handle_irq(&self) {}
}
