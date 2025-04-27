use core::any::Any;

use alloc::boxed::Box;
use smoltcp::phy::DeviceCapabilities;

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
