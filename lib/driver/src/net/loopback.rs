use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use arch::mm::tlb_shootdown_all;
use smoltcp::phy::{DeviceCapabilities, Medium};

use super::{DevError, DevResult, EthernetAddress, NetDevice, netbuf::NetBufPtrOps};

/// The loopback interface operates at the network layer and handles the packets
/// directly at the IP level. Consequently, packets sent to 127.0.0.1 do not
/// include Ethernet headers because they never actually touch the physical
/// network hardware, which is necessary for Ethernet frame encapsulation
#[derive(Debug)]
pub struct LoopbackDev {
    queue: VecDeque<Vec<u8>>,
}

impl LoopbackDev {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            queue: VecDeque::with_capacity(256),
        })
    }
}

static mut DEV_CAP: Option<DeviceCapabilities> = None;

impl NetDevice for LoopbackDev {
    fn fill_capabilities(&self, out: &mut DeviceCapabilities) {
        out.medium = Medium::Ip;
        out.max_burst_size = None;
        out.max_transmission_unit = 65535;
    }

    #[inline]
    #[allow(static_mut_refs)]
    fn capabilities(&self) -> DeviceCapabilities {
        // unsafe {
        //     log::debug!("test4");
        //     // DEV_CAP = Some(DeviceCapabilities::default());
        //     // log::debug!("DEV_CAP: {:#x}", core::ptr::addr_of!(DEV_CAP) as usize);
        //     log::debug!("test5");

        //     let sp: usize;
        //     core::arch::asm!("move {}, $sp", out(reg) sp);
        //     log::debug!("stack pointer: {:#x}", sp);

        //     let mut cap = DeviceCapabilities::default();
        //     log::debug!("test6");
        //     cap.medium = Medium::Ip;
        //     cap.max_burst_size = None;
        //     cap.max_transmission_unit = 65535;

        //     log::debug!("cap: {:#x}", core::ptr::addr_of!(cap) as usize);
        //     cap
        // }

        log::debug!(
            "DeviceCapabilities size: {}",
            core::mem::size_of::<DeviceCapabilities>()
        );

        log::debug!(
            "DeviceCapabilities align: {}",
            core::mem::align_of::<DeviceCapabilities>()
        );

        let mut cap = smoltcp::phy::DeviceCapabilities::default();
        self.fill_capabilities(&mut cap);
        log::debug!("cap addr: {:x}", &cap as *const _ as usize);
        log::debug!("cap.medium addr: {:x}", &cap.medium as *const _ as usize);
        log::debug!(
            "cap.max_transmission_unit addr: {:x}",
            &cap.max_transmission_unit as *const _ as usize
        );
        log::debug!(
            "cap.max_burst_size addr: {:x}",
            &cap.max_burst_size as *const _ as usize
        );
        log::debug!(
            "cap.checksum addr: {:x}",
            &cap.checksum as *const _ as usize
        );

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
        let mut buffer = vec![0; size];
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
