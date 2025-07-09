use alloc::boxed::Box;

use smoltcp::phy::DeviceCapabilities;

use netpool::NetBufPtrOps;

pub mod loopback;
pub mod netpool;
pub mod virtnet;

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

impl DevError {
    /// Converts a `virtio_drivers::Error` into a `DevError`.
    pub const fn from_virtio_error(e: virtio_drivers::Error) -> Self {
        use virtio_drivers::Error::*;
        match e {
            QueueFull => DevError::BadState,
            NotReady => DevError::Again,
            WrongToken => DevError::BadState,
            AlreadyUsed => DevError::AlreadyExists,
            InvalidParam => DevError::InvalidParam,
            DmaError => DevError::NoMemory,
            IoError => DevError::Io,
            Unsupported => DevError::Unsupported,
            ConfigSpaceTooSmall => DevError::BadState,
            ConfigSpaceMissing => DevError::BadState,
            _ => DevError::BadState,
        }
    }
}

/// A special `Result` type for device operations.
pub type DevResult<T = ()> = Result<T, DevError>;

/// A 48-bit Ethernet (MAC) address.
pub struct EthernetAddress(pub [u8; 6]);

pub trait NetDevice: Sync + Send {
    /// Returns the capabilities of the network device.
    fn capabilities(&self) -> DeviceCapabilities;

    /// Returns the ethernet address of the device.
    fn mac_address(&self) -> EthernetAddress;

    /// Returns whether the device can transmit packets.
    fn can_transmit(&self) -> bool;

    /// Returns whether the device can receive packets.
    fn can_receive(&self) -> bool;

    /// Returns the size of the receive queue.
    fn rx_queue_size(&self) -> usize;

    /// Returns the size of the transmit queue.
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

    /// Returns an available transmit buffer of size `size` of the device.
    fn take_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn NetBufPtrOps>>;
}
