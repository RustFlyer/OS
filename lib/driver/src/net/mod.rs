use core::any::Any;

use alloc::{boxed::Box, string::ToString};
use fdt::Fdt;
use mm::address::PhysAddr;
use smoltcp::phy::DeviceCapabilities;
use virtio_drivers::transport::DeviceType;

use crate::{
    device::{DevId, DeviceMajor, DeviceMeta},
    manager::probe_mmio_device,
};

pub fn probe_virtio_net(root: &Fdt) -> Option<DeviceMeta> {
    let device_tree = root;
    let mut net_meta = None;
    for node in device_tree.find_all_nodes("/soc/virtio_mmio") {
        log::debug!("[probe_virtio_net] probe node {}", node.name);
        for reg in node.reg()? {
            let mmio_base_paddr = PhysAddr::new(reg.starting_address as usize);
            let mmio_size = reg.size?;
            log::debug!("[probe_virtio_net] probe reg {:?}", reg);
            if probe_mmio_device(
                mmio_base_paddr.to_va_kernel().to_usize() as *mut u8,
                mmio_size,
                Some(DeviceType::Network),
            )
            .is_some()
            {
                log::debug!("[probe_virtio_net] find a net device");
                net_meta = {
                    Some(DeviceMeta {
                        mmio_base: mmio_base_paddr.to_usize(),
                        mmio_size,
                        name: "virtio-blk".to_string(),
                        dtype: DeviceType::Network,
                        dev_id: DevId {
                            major: DeviceMajor::Net,
                            minor: 0,
                        },
                        irq_no: None,
                    })
                }
            }
            if net_meta.is_some() {
                break;
            }
        }
    }
    if net_meta.is_none() {
        log::warn!("No virtio net device found");
    }
    net_meta
}

/// The error type for device operation failures.
#[derive(Debug)]
pub enum DevError {
    /// An entity already exists.
    AlreadyExists,
    /// Try again, for non-blocking APIs.
    Again,
    /// Bad internal state.
    BadState,
    /// Invalid parameter/argument.
    InvalidParam,
    /// Input/output error.
    Io,
    /// Not enough space/cannot allocate memory (DMA).
    NoMemory,
    /// Device or resource is busy.
    ResourceBusy,
    /// This operation is unsupported or unimplemented.
    Unsupported,
}

/// A specialized `Result` type for device operations.
pub type DevResult<T = ()> = Result<T, DevError>;

pub struct EthernetAddress(pub [u8; 6]);
pub trait NetDevice: Sync + Send {
    fn capabilities(&self) -> DeviceCapabilities;
    /// The ethernet address of the NIC.
    fn mac_address(&self) -> EthernetAddress;

    /// Whether can transmit packets.
    fn can_transmit(&self) -> bool;

    /// Whether can receive packets.
    fn can_receive(&self) -> bool;

    /// Size of the receive queue.
    fn rx_queue_size(&self) -> usize;

    /// Size of the transmit queue.
    fn tx_queue_size(&self) -> usize;

    /// Gives back the `rx_buf` to the receive queue for later receiving.
    ///
    /// `rx_buf` should be the same as the one returned by
    /// [`NetDriverOps::receive`].
    fn recycle_rx_buffer(&mut self, rx_buf: Box<dyn NetBufPtrOps>) -> DevResult;

    /// Poll the transmit queue and gives back the buffers for previous
    /// transmiting. returns [`DevResult`].
    fn recycle_tx_buffers(&mut self) -> DevResult;

    /// Transmits a packet in the buffer to the network, without blocking,
    /// returns [`DevResult`].
    fn transmit(&mut self, tx_buf: Box<dyn NetBufPtrOps>) -> DevResult;

    /// Receives a packet from the network and store it in the [`NetBuf`],
    /// returns the buffer.
    ///
    /// Before receiving, the driver should have already populated some buffers
    /// in the receive queue by [`NetDriverOps::recycle_rx_buffer`].
    ///
    /// If currently no incomming packets, returns an error with type
    /// [`DevError::Again`].
    fn receive(&mut self) -> DevResult<Box<dyn NetBufPtrOps>>;

    /// Allocate a memory buffer of a specified size for network transmission,
    /// returns [`DevResult`]
    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn NetBufPtrOps>>;
}
pub trait NetBufPtrOps: Any {
    fn packet(&self) -> &[u8];
    fn packet_mut(&mut self) -> &mut [u8];
    fn packet_len(&self) -> usize;
}
