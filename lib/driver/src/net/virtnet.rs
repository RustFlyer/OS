use core::{any::Any, fmt::Debug};

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

use super::{
    DevError, DevResult, EthernetAddress, NetDevice, as_dev_err,
    netbuf::{NetBufPtr, NetBufPtrOps},
};

const QS: usize = 32;
const NET_BUF_LEN: usize = 1526;

pub fn create_virt_net_dev(transport: MmioTransport<'static>) -> DevResult<Box<VirtIoNetDevImpl>> {
    const NONE_BUF: Option<Box<NetBuf>> = None;

    let inner =
        VirtIONetRaw::<VirtHalImpl, MmioTransport, QS>::new(transport).map_err(as_dev_err)?;
    let rx_buffers = [NONE_BUF; QS];
    let tx_buffers = [NONE_BUF; QS];
    let buf_pool = NetBufPool::new(2 * QS, NET_BUF_LEN)?;
    let free_tx_bufs = Vec::with_capacity(QS);

    let mut dev = VirtIoNetDevImpl {
        rx_buffers,
        inner,
        tx_buffers,
        free_tx_bufs,
        buf_pool,
    };

    log::debug!("[create_virt_net_dev] {:?}", dev);

    // 1. Fill all rx buffers.
    for (i, rx_buf_place) in dev.rx_buffers.iter_mut().enumerate() {
        let mut rx_buf = dev.buf_pool.alloc_boxed().ok_or(DevError::NoMemory)?;
        let token = unsafe {
            dev.inner
                .receive_begin(rx_buf.raw_buf_mut())
                .map_err(as_dev_err)?
        };
        assert_eq!(token, i as u16);
        *rx_buf_place = Some(rx_buf);
    }

    // 2. Allocate all tx buffers.
    for _ in 0..QS {
        let mut tx_buf = dev.buf_pool.alloc_boxed().ok_or(DevError::NoMemory)?;
        // Fill header
        let hdr_len = dev
            .inner
            .fill_buffer_header(tx_buf.raw_buf_mut())
            .or(Err(DevError::InvalidParam))?;
        tx_buf.set_header_len(hdr_len);
        dev.free_tx_bufs.push(tx_buf);
    }

    Ok(Box::new(dev))
}

pub struct VirtIoNetDev<T: Transport, const QS: usize> {
    pub(crate) rx_buffers: [Option<Box<NetBuf>>; QS],
    pub(crate) tx_buffers: [Option<Box<NetBuf>>; QS],
    pub(crate) free_tx_bufs: Vec<Box<NetBuf>>,
    pub(crate) buf_pool: Arc<NetBufPool>,
    pub(crate) inner: VirtIONetRaw<VirtHalImpl, T, QS>,
}

pub type VirtIoNetDevImpl = VirtIoNetDev<MmioTransport<'static>, 32>;

impl Debug for VirtIoNetDevImpl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtIoNetDevImpl")
            .field("rx_buffers", &self.rx_buffers)
            .field("tx_buffers", &self.tx_buffers)
            .field("free_tx_bufs", &self.free_tx_bufs)
            .finish()
    }
}

unsafe impl<T: Transport, const QS: usize> Send for VirtIoNetDev<T, QS> {}
unsafe impl<T: Transport, const QS: usize> Sync for VirtIoNetDev<T, QS> {}

impl<T: Transport + 'static, const QS: usize> NetDevice for VirtIoNetDev<T, QS> {
    fn fill_capabilities(&self, out: &mut DeviceCapabilities) {
        todo!()
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn super::NetBufPtrOps>> {
        // 0. Allocate a buffer from the queue.
        let mut net_buf = self.free_tx_bufs.pop().ok_or(DevError::NoMemory)?;
        let pkt_len = size;

        // 1. Check if the buffer is large enough.
        let hdr_len = net_buf.header_len();
        if hdr_len + pkt_len > net_buf.capacity() {
            return Err(DevError::InvalidParam);
        }
        net_buf.set_packet_len(pkt_len);

        // 2. Return the buffer.
        Ok(net_buf.into_buf_ptr())
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
        !self.free_tx_bufs.is_empty() && self.inner.can_send()
    }

    fn can_receive(&self) -> bool {
        self.inner.poll_receive().is_some()
    }

    fn rx_queue_size(&self) -> usize {
        QS
    }

    fn tx_queue_size(&self) -> usize {
        QS
    }

    fn recycle_rx_buffer(&mut self, rx_buf: Box<dyn super::NetBufPtrOps>) -> DevResult {
        let rx_buf =
            unsafe { core::mem::transmute::<Box<dyn NetBufPtrOps>, Box<dyn Any + Send>>(rx_buf) };
        let mut rx_buf = unsafe { NetBuf::from_buf_ptr(rx_buf.downcast::<NetBufPtr>().unwrap()) };
        // Safe because we take the ownership of `rx_buf` back to `rx_buffers`,
        // it lives as long as the queue.
        let new_token = unsafe {
            self.inner
                .receive_begin(rx_buf.raw_buf_mut())
                .map_err(as_dev_err)?
        };
        // `rx_buffers[new_token]` is expected to be `None` since it was taken
        // away at `Self::receive()` and has not been added back.
        if self.rx_buffers[new_token as usize].is_some() {
            return Err(DevError::BadState);
        }
        self.rx_buffers[new_token as usize] = Some(rx_buf);
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> DevResult {
        while let Some(token) = self.inner.poll_transmit() {
            let tx_buf = self.tx_buffers[token as usize]
                .take()
                .ok_or(DevError::BadState)?;
            unsafe {
                self.inner
                    .transmit_complete(token, tx_buf.packet_with_header())
                    .map_err(as_dev_err)?;
            }
            // Recycle the buffer.
            self.free_tx_bufs.push(tx_buf);
        }
        Ok(())
    }

    fn transmit(&mut self, tx_buf: Box<dyn super::NetBufPtrOps>) -> DevResult {
        let tx_buf =
            unsafe { core::mem::transmute::<Box<dyn NetBufPtrOps>, Box<dyn Any + Send>>(tx_buf) };
        // 0. prepare tx buffer.
        let tx_buf = unsafe { NetBuf::from_buf_ptr(tx_buf.downcast::<NetBufPtr>().unwrap()) };
        // 1. transmit packet.
        let token = unsafe {
            self.inner
                .transmit_begin(tx_buf.packet_with_header())
                .map_err(as_dev_err)?
        };
        self.tx_buffers[token as usize] = Some(tx_buf);
        Ok(())
    }

    fn receive(&mut self) -> DevResult<Box<dyn super::NetBufPtrOps>> {
        if let Some(token) = self.inner.poll_receive() {
            log::warn!("[VirtioNetDev::receive] token {}", token);
            let mut rx_buf = self.rx_buffers[token as usize]
                .take()
                .ok_or(DevError::BadState)?;

            let (hdr_len, pkt_len) = unsafe {
                self.inner
                    .receive_complete(token, rx_buf.raw_buf_mut())
                    .map_err(as_dev_err)?
            };
            rx_buf.set_header_len(hdr_len);
            rx_buf.set_packet_len(pkt_len);

            Ok(rx_buf.into_buf_ptr())
        } else {
            Err(DevError::Again)
        }
    }
}
