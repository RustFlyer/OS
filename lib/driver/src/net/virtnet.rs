use alloc::{boxed::Box, sync::Arc, vec::Vec};
use smoltcp::phy::{DeviceCapabilities, Medium};
use virtio_drivers::{
    device::net::VirtIONetRaw,
    transport::{Transport, mmio::MmioTransport},
};

use crate::{
    hal::VirtHalImpl,
    net::{netbuf::NetBuf, netpool::NetBufPool},
};

use super::{DevResult, EthernetAddress, NetDevice};

const QS: usize = 32;
const NET_BUF_LEN: usize = 1526;

pub fn create_virt_net_dev(transport: MmioTransport) -> DevResult<Box<VirtIoNetDevImpl>> {
    const NONE_BUF: Option<Box<NetBuf>> = None;
    let inner = VirtIONetRaw::<VirtHalImpl, MmioTransport, QS>::new(transport)
        .expect("can not create VirtIONetRaw");
    let rx_buffers = [NONE_BUF; QS];
    let tx_buffers = [NONE_BUF; QS];
    let buf_pool = NetBufPool::new(2 * QS, NET_BUF_LEN)?;
    let free_tx_bufs = Vec::with_capacity(QS);

    let dev = VirtIoNetDevImpl {
        rx_buffers,
        inner,
        tx_buffers,
        free_tx_bufs,
        buf_pool,
    };

    Ok(Box::new(dev))
}

pub struct VirtIoNetDev<T: Transport, const QS: usize> {
    pub(crate) rx_buffers: [Option<Box<NetBuf>>; QS],
    pub(crate) tx_buffers: [Option<Box<NetBuf>>; QS],
    pub(crate) free_tx_bufs: Vec<Box<NetBuf>>,
    pub(crate) buf_pool: Arc<NetBufPool>,
    pub(crate) inner: VirtIONetRaw<VirtHalImpl, T, QS>,
}

pub type VirtIoNetDevImpl = VirtIoNetDev<MmioTransport, 32>;

unsafe impl<T: Transport, const QS: usize> Send for VirtIoNetDev<T, QS> {}
unsafe impl<T: Transport, const QS: usize> Sync for VirtIoNetDev<T, QS> {}

impl<T: Transport + 'static, const QS: usize> NetDevice for VirtIoNetDev<T, QS> {
    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn super::NetBufPtrOps>> {
        todo!()
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1000;
        caps.max_burst_size = None;
        caps
    }

    fn mac_address(&self) -> super::EthernetAddress {
        EthernetAddress(self.inner.mac_address())
    }

    fn can_transmit(&self) -> bool {
        todo!()
    }

    fn can_receive(&self) -> bool {
        todo!()
    }

    fn rx_queue_size(&self) -> usize {
        todo!()
    }

    fn tx_queue_size(&self) -> usize {
        todo!()
    }

    fn recycle_rx_buffer(&mut self, rx_buf: Box<dyn super::NetBufPtrOps>) -> DevResult {
        todo!()
    }

    fn recycle_tx_buffers(&mut self) -> DevResult {
        todo!()
    }

    fn transmit(&mut self, tx_buf: Box<dyn super::NetBufPtrOps>) -> DevResult {
        todo!()
    }

    fn receive(&mut self) -> DevResult<Box<dyn super::NetBufPtrOps>> {
        todo!()
    }
}
