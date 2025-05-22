use super::hal::VirtHalImpl;
use crate::BlockDevice;
use config::device::{BLOCK_SIZE, DEV_SIZE, VIRTIO0};
use mutex::SpinNoIrqLock;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

pub struct VirtBlkDevice(SpinNoIrqLock<VirtIOBlk<VirtHalImpl, MmioTransport>>);

unsafe impl Sync for VirtBlkDevice {}
unsafe impl Send for VirtBlkDevice {}

impl BlockDevice for VirtBlkDevice {
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

impl VirtBlkDevice {
    pub fn new() -> Self {
        unsafe {
            let header = &mut *(VIRTIO0 as *mut VirtIOHeader);
            let blk = VirtIOBlk::<VirtHalImpl, MmioTransport>::new(
                MmioTransport::new(header.into()).unwrap(),
            );
            Self(SpinNoIrqLock::new(blk.unwrap()))
        }
    }

    pub fn new_from(transport: MmioTransport) -> Self {
        let blk = VirtIOBlk::<VirtHalImpl, MmioTransport>::new(transport);
        Self(SpinNoIrqLock::new(blk.unwrap()))
    }
}
