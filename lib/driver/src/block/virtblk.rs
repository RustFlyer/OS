use crate::BlockDevice;
use crate::device::OSDevice;
use crate::hal::VirtHalImpl;
use alloc::sync::Arc;
use config::device::{BLOCK_SIZE, DEV_SIZE, VIRTIO0};
use mutex::SpinNoIrqLock;
use virtio_drivers::transport::Transport;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{mmio::MmioTransport, pci::PciTransport},
};

pub struct VirtBlkDevice<T: Transport>(SpinNoIrqLock<VirtIOBlk<VirtHalImpl, T>>);

impl<T: Transport> VirtBlkDevice<T> {
    pub fn new(transport: T) -> Self {
        let blk = VirtIOBlk::<VirtHalImpl, T>::new(transport);
        if let Err(e) = blk {
            log::error!("blk: {:?}", e);
        }
        Self(SpinNoIrqLock::new(blk.unwrap()))
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
        let res = self.0.lock().read_blocks(block_id, buf);
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
        let res = self.0.lock().write_blocks(block_id, buf);
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
        let res = self.0.lock().read_blocks(block_id, buf);
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
        let res = self.0.lock().write_blocks(block_id, buf);
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
        todo!()
    }

    fn init(&self) {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}

impl OSDevice for VirtBlkDevice<PciTransport> {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        todo!()
    }

    fn init(&self) {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}
