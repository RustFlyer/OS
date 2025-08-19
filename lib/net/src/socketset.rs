use alloc::{sync::Arc, vec};
use mutex::{SpinNoIrqLock, new_share_mutex};
use smoltcp::{
    iface::{SocketHandle, SocketSet},
    socket::{self, AnySocket, tcp::SocketBuffer},
    wire::{IpProtocol, IpVersion},
};

use crate::ETH0;

pub const TCP_RX_BUF_LEN: usize = 64 * 1024;
pub const TCP_TX_BUF_LEN: usize = 64 * 1024;
pub const UDP_RX_BUF_LEN: usize = 64 * 1024;
pub const UDP_TX_BUF_LEN: usize = 64 * 1024;
pub const LISTEN_QUEUE_SIZE: usize = 512;

/// `SocketSet` is a collection of sockets that contain multiple different types
/// of sockets (such as TCP, UDP, ICMP, etc.). It provides a mechanism to manage
/// and operate these sockets, including polling socket status, processing data
/// transmission and reception, etc.
///
/// It is similar to `FdTable` and `SocketHandle` is similar to `fd`
pub(crate) struct SocketSetWrapper(pub(crate) Arc<SpinNoIrqLock<SocketSet<'static>>>);

/// Tcp Socket
///
/// A TCP socket may passively listen for connections or actively connect to
/// another endpoint. Note that, for listening sockets, there is no "backlog";
/// to be able to simultaneously accept several connections, as many sockets must
/// be allocated, or any new connection attempts will be reset.
type SmolTcpSocket<'a> = socket::tcp::Socket<'a>;

/// Udp Socket
///
/// A UDP socket is bound to a specific endpoint, and owns transmit and receive
/// packet buffers.
type SmolUdpSocket<'a> = socket::udp::Socket<'a>;

/// Udp Packet Buffer, a ring buffer
type SmolUdpPacketBuffer<'a> = socket::udp::PacketBuffer<'a>;

/// Raw Socket
type SmolRawSocket<'a> = socket::raw::Socket<'a>;

/// Raw Packet Buffer, a ring buffer
type SmolRawPacketBuffer<'a> = socket::raw::PacketBuffer<'a>;

/// Udp Packet metadata
/// ```rust
/// pub struct UdpMetadata {
/// pub endpoint: Endpoint,
/// pub local_address: Option<Address>,
/// pub meta: PacketMeta,
/// }
/// ```
type SmolUdpPacketMetadata = socket::udp::PacketMetadata;

type SmolRawPacketMetadata = socket::raw::PacketMetadata;

/// `SmolInstant` is a representation of an absolute time value.
///
/// The Instant type is a wrapper around a i64 value that represents a number of
/// microseconds, monotonically increasing since an arbitrary moment in time,
/// such as system startup.
///
/// - A value of 0 is inherently arbitrary.
/// - A value less than 0 indicates a time before the starting point
type SmolInstant = smoltcp::time::Instant;

impl SocketSetWrapper {
    /// Creates a new `SocketSetWrapper`. In fact, this function is only called
    /// by `SOCKET_SET`.
    pub fn new() -> Self {
        Self(new_share_mutex(SocketSet::new(vec![])))
    }

    /// Creates a new tcp socket consisting of `tcp_rx_buffer` and `tcp_tx_buffer`.
    pub fn new_tcp_socket() -> SmolTcpSocket<'static> {
        let tcp_rx_buffer = SocketBuffer::new(vec![0; TCP_RX_BUF_LEN]);
        let tcp_tx_buffer = SocketBuffer::new(vec![0; TCP_TX_BUF_LEN]);
        SmolTcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
    }

    /// Creates a new udp socket consisting of `udp_rx_buffer` and `udp_tx_buffer`.
    pub fn new_udp_socket() -> SmolUdpSocket<'static> {
        let udp_rx_buffer = SmolUdpPacketBuffer::new(vec![SmolUdpPacketMetadata::EMPTY; 8], vec![
                0;
                UDP_RX_BUF_LEN
            ]);
        let udp_tx_buffer = SmolUdpPacketBuffer::new(vec![SmolUdpPacketMetadata::EMPTY; 8], vec![
                0;
                UDP_TX_BUF_LEN
            ]);
        SmolUdpSocket::new(udp_rx_buffer, udp_tx_buffer)
    }

    /// Creates a new raw socket consisting of `tcp_rx_buffer` and `tcp_tx_buffer`.
    pub fn new_raw_socket(ip_protocol: IpProtocol, version: IpVersion) -> SmolRawSocket<'static> {
        let raw_rx_buffer = SmolRawPacketBuffer::new(vec![SmolRawPacketMetadata::EMPTY; 8], vec![
                0;
                TCP_RX_BUF_LEN
            ]);
        let raw_tx_buffer = SmolRawPacketBuffer::new(vec![SmolRawPacketMetadata::EMPTY; 8], vec![
                0;
                TCP_TX_BUF_LEN
            ]);
        SmolRawSocket::new(version, ip_protocol, raw_rx_buffer, raw_tx_buffer)
    }

    /// Like `fdtable`, this function can receive a `socket` and add it into `Socket_Set`.
    /// A `SocketHandle` will be returned as `Fd` in `Fdtable` when the `socket` is added
    /// successfully.
    pub fn add<T: AnySocket<'static>>(&self, socket: T) -> SocketHandle {
        let handle = self.0.lock().add(socket);
        handle
    }

    /// Like `fdtable`, this function can receive a `handle` and remove its socket from
    /// the socket_set.
    pub fn remove(&self, handle: SocketHandle) {
        self.0.lock().remove(handle);
    }

    /// This function exposes its socket to caller as a closure. Just ensure the socket lock
    /// in a certain range.
    pub fn with_socket_mut<T: AnySocket<'static>, R, F>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        // log::debug!("[with_socket_mut] {:?}", handle);
        let mut set = self.0.lock();
        let socket = set.get_mut(handle);
        f(socket)
    }

    /// The core function of net module. Poll and process data packets in waiting list.
    pub fn poll_interfaces(&self) -> SmolInstant {
        // {
        //     let lock = self.0.lock();
        //     lock.iter()
        //         .for_each(|s| log::debug!("[poll_interfaces] {}", s.0));
        // }
        ETH0.get().unwrap().poll(self.0.clone())
    }

    /// Different from `poll_interfaces`, it checks time and decides whether to poll.
    pub fn check_poll(&self, timestamp: SmolInstant) {
        ETH0.get().unwrap().check_poll(timestamp, &self.0)
    }
}
