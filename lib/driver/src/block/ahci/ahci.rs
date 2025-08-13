use core::{cell::SyncUnsafeCell, ops::DerefMut};

use alloc::{boxed::Box, string::ToString, sync::Arc};
use config::device::BLOCK_SIZE;
use mutex::SpinNoIrqLock;

use crate::{
    BlockDevice,
    block::ahci::drv_ahci::{ahci_sata_read_common, ahci_sata_write_common},
    device::{OSDevId, OSDevice, OSDeviceKind, OSDeviceMajor, OSDeviceMeta},
};

use super::{
    drv_ahci::ahci_init,
    libahci::{self, ahci_blk_dev, ahci_device, ahci_ioport},
};

pub struct AHCI {
    meta: OSDeviceMeta,
    device: SpinNoIrqLock<ahci_device>,
}

unsafe impl Send for AHCI {}
unsafe impl Sync for AHCI {}

impl AHCI {
    pub fn new(base_address: usize, size: usize, interrupt_number: usize) -> AHCI {
        let nahci_ioport = ahci_ioport {
            port_mmio: 0,
            cmd_slot: core::ptr::null::<u32>() as *mut libahci::ahci_cmd_hdr,
            cmd_slot_dma: 0,
            rx_fis: 0,
            rx_fis_dma: 0,
            cmd_tbl: 0,
            cmd_tbl_dma: 0,
            cmd_tbl_sg: core::ptr::null::<u32>() as *mut libahci::ahci_sg,
        };

        let ahci_blk_dev = ahci_blk_dev {
            lba48: false,
            lba: 0,
            blksz: 0,
            queue_depth: 0,
            product: [0; 41],
            serial: [0; 21],
            revision: [0; 9],
        };

        let ahci_dev = ahci_device {
            mmio_base: base_address as u64,
            flags: 0,
            cap: 0,
            cap2: 0,
            version: 0,
            port_map: 0,
            pio_mask: 0,
            udma_mask: 0,
            n_ports: 0,
            port_map_linkup: 0,
            port: [nahci_ioport; 32],
            port_idx: 0,
            blk_dev: ahci_blk_dev,
        };

        AHCI {
            meta: OSDeviceMeta {
                dev_id: OSDevId {
                    major: OSDeviceMajor::Block,
                    minor: 1,
                },
                name: "snps,spear-ahci".to_string(),
                mmio_base: base_address,
                mmio_size: size,
                irq_no: Some(interrupt_number),
                dtype: OSDeviceKind::SDMMC,
                pci_bar: None,
                pci_bdf: None,
                pci_ids: None,
            },
            device: SpinNoIrqLock::new(ahci_dev),
        }
    }
}

impl OSDevice for AHCI {
    fn meta(&self) -> &OSDeviceMeta {
        &self.meta
    }

    fn init(&self) {
        log::debug!("try to init ahci");
        let mut dev = self.device.lock();
        dev.mmio_base = 0x8000_0000_400e_0000;
        ahci_init(dev.deref_mut());
    }

    fn handle_irq(&self) {
        log::info!("ahci handle_irq");
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        Some(self)
    }
}

impl BlockDevice for AHCI {
    fn size(&self) -> u64 {
        let dev = self.device.lock();
        dev.blk_dev.lba * dev.blk_dev.blksz as u64
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn read(&self, block_id: usize, buf: &mut [u8]) {
        assert!(buf.len() == BLOCK_SIZE);
        let fsblocksize = BLOCK_SIZE;

        let dev = self.device.lock();
        let sector_size = dev.blk_dev.blksz as usize;

        let sectors_needed = (fsblocksize + sector_size - 1) / sector_size;
        let start_sector = (block_id * fsblocksize) / sector_size;

        log::warn!(
            "read sector_size: {sector_size:#x}, sectors_needed: {sectors_needed:#x}, start_sector: {start_sector:#x}, block_id: {block_id:#x}"
        );
        let result = ahci_sata_read_common(
            &*dev,
            start_sector as u64,
            sectors_needed as u32,
            buf.as_mut_ptr(),
        );
    }

    fn write(&self, block_id: usize, buf: &[u8]) {
        assert!(buf.len() == BLOCK_SIZE);
        let fsblocksize = BLOCK_SIZE;

        let dev = self.device.lock();
        let sector_size = dev.blk_dev.blksz as usize;

        let sectors_needed = (fsblocksize + sector_size - 1) / sector_size;
        let start_sector = (block_id * fsblocksize) / sector_size;

        // log::debug!(
        //     "write sector_size: {sector_size:#x}, sectors_needed: {sectors_needed:#x}, start_sector: {start_sector:#x}, block_id: {block_id:#x}"
        // );
        let result = ahci_sata_write_common(
            &*dev,
            start_sector as u64,
            sectors_needed as u32,
            buf.as_ptr() as *mut u8,
        );
    }
}
