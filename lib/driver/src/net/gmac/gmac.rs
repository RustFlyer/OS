use core::ptr::null;

use alloc::{
    boxed::Box,
    collections::VecDeque,
    string::ToString,
    vec::{self, Vec},
};
use arch::mm::tlb_shootdown_all;
use mutex::SpinNoIrqLock;
use smoltcp::phy::{DeviceCapabilities, Medium};
use virtio_drivers::transport::DeviceType;

use crate::{
    device::{OSDevId, OSDeviceKind, OSDeviceMajor, OSDeviceMeta},
    net::{DevError, DevResult, EthernetAddress, NetDevice, netbuf::NetBufPtrOps},
};

use super::eth_defs::{DmaDesc, net_device};

/// The loopback interface operates at the network layer and handles the packets
/// directly at the IP level. Consequently, packets sent to 127.0.0.1 do not
/// include Ethernet headers because they never actually touch the physical
/// network hardware, which is necessary for Ethernet frame encapsulation
#[derive(Debug)]
pub struct Gmac {
    meta: OSDeviceMeta,
    device: SpinNoIrqLock<net_device>,
    queue: VecDeque<Vec<u8>>,
}

impl Gmac {
    pub fn new(base_address: usize, size: usize, interrupt_number: usize) -> Box<Self> {
        let device = net_device {
            parent: null::<*mut u8>() as *mut u8,
            iobase: 0,
            MacAddr: [0; 6],
            MacBase: 0,
            DmaBase: 0,
            PhyBase: 0,
            Version: 0,
            TxBusy: 0,
            TxNext: 0,
            RxBusy: 0,
            TxDesc: [null::<*mut DmaDesc>() as *mut DmaDesc; 128],
            RxDesc: [null::<*mut DmaDesc>() as *mut DmaDesc; 128],
            TxBuffer: [0; 128],
            RxBuffer: [0; 128],
            rx_packets: 0,
            tx_packets: 0,
            rx_bytes: 0,
            tx_bytes: 0,
            rx_errors: 0,
            tx_errors: 0,
            advertising: 0,
            LinkStatus: 0,
            DuplexMode: 0,
            Speed: 0,
        };
        Box::new(Self {
            meta: OSDeviceMeta {
                dev_id: OSDevId {
                    major: OSDeviceMajor::Net,
                    minor: 1,
                },
                name: "snps,spear-ahci".to_string(),
                mmio_base: base_address,
                mmio_size: size,
                irq_no: Some(interrupt_number),
                dtype: OSDeviceKind::Virtio(DeviceType::Network),
                pci_bar: None,
                pci_bdf: None,
                pci_ids: None,
            },
            device: SpinNoIrqLock::new(device),
            queue: VecDeque::with_capacity(256),
        })
    }
}

unsafe impl Send for Gmac {}
unsafe impl Sync for Gmac {}

impl NetDevice for Gmac {
    fn fill_capabilities(&self, out: &mut DeviceCapabilities) {
        out.medium = Medium::Ip;
        out.max_burst_size = None;
        out.max_transmission_unit = 65535;
    }

    #[inline]
    #[allow(static_mut_refs)]
    fn capabilities(&self) -> DeviceCapabilities {
        let mut cap = DeviceCapabilities::default();
        cap.medium = Medium::Ip;
        cap.max_burst_size = None;
        cap.max_transmission_unit = 65535;

        cap
    }

    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    }

    fn can_transmit(&self) -> bool {
        true
    }

    fn can_receive(&self) -> bool {
        !self.queue.is_empty()
    }

    fn rx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn tx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn recycle_rx_buffer(&mut self, _rx_buf: Box<dyn NetBufPtrOps>) -> DevResult {
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> DevResult {
        Ok(())
    }

    fn transmit(&mut self, tx_buf: Box<dyn NetBufPtrOps>) -> DevResult {
        let data = tx_buf.packet().to_vec();
        // log::warn!("[NetDriverOps::transmit] now transmit {} bytes", data.len());
        self.queue.push_back(data);
        Ok(())
    }

    fn receive(&mut self) -> DevResult<Box<dyn NetBufPtrOps>> {
        if let Some(buf) = self.queue.pop_front() {
            // log::warn!(
            //     "[NetDriverOps::receive] now receive {} bytes from LoopbackDev.queue",
            //     buf.len()
            // );
            Ok(Box::new(SimpleNetBuf(buf)))
        } else {
            Err(DevError::Again)
        }
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn NetBufPtrOps>> {
        let mut buffer = alloc::vec![0; size];
        buffer.resize(size, 0);
        Ok(Box::new(SimpleNetBuf(buffer)))
    }
}

#[derive(Debug)]
struct SimpleNetBuf(Vec<u8>);

impl NetBufPtrOps for SimpleNetBuf {
    fn packet(&self) -> &[u8] {
        self.0.as_slice()
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    fn packet_len(&self) -> usize {
        self.0.len()
    }
}
