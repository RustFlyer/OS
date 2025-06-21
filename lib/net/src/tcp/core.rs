use alloc::vec::Vec;
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU8},
};

use smoltcp::{
    iface::SocketHandle,
    socket::tcp::{self, State},
    wire::IpEndpoint,
};

use mutex::ShareMutex;

use crate::{SOCKET_SET, poll_interfaces, tcp::SHUT_RDWR};

/// A TCP socket that provides POSIX-like APIs.
///
/// - [`connect`] is for TCP clients.
/// - [`bind`], [`listen`], and [`accept`] are for TCP servers.
/// - Other methods are for both TCP clients and servers.
///
/// [`connect`]: TcpSocket::connect
/// [`bind`]: TcpSocket::bind
/// [`listen`]: TcpSocket::listen
/// [`accept`]: TcpSocket::accept
#[derive(Debug)]
pub struct TcpSocket {
    /// Manages the state of the socket using an atomic u8 for lock-free
    /// management.
    pub(crate) state: AtomicU8,
    /// Indicates whether the read or write directions of the socket have been
    /// explicitly shut down. This does not represent the connection state.
    /// Once shut down, the socket cannot be reconnected via `connect`.
    pub(crate) shutdown: UnsafeCell<u8>,
    /// An optional handle to the socket, managed within an UnsafeCell for
    /// interior mutability.
    pub(crate) handle: UnsafeCell<Option<SocketHandle>>,
    /// Stores the local IP endpoint of the socket, using UnsafeCell for
    /// interior mutability.
    pub(crate) local_addr: UnsafeCell<IpEndpoint>,
    /// Stores the peer IP endpoint of the socket, using UnsafeCell for interior
    /// mutability.
    pub(crate) peer_addr: UnsafeCell<IpEndpoint>,
    /// Indicates whether the socket is in non-blocking mode, using an atomic
    /// boolean for thread-safe access.
    pub(crate) nonblock: AtomicBool,
    pub(crate) listen_handles: ShareMutex<Vec<SocketHandle>>,
}

unsafe impl Sync for TcpSocket {}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        log::info!("[TcpSocket::Drop]");
        self.shutdown(SHUT_RDWR).ok();
        // Safe because we have mut reference to `self`.
        // self.listen_handles.lock().clear();
        if let Some(handle) = unsafe { self.handle.get().read() } {
            let mut cnt = 0;
            loop {
                let is_close = SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |sock| {
                    sock.state() == State::Closed
                });
                if is_close || cnt > 10000 {
                    log::error!("[shutdown] poll cnt is {}", cnt);
                    break;
                }
                poll_interfaces();
                cnt += 1;
            }
            // LISTEN_TABLE.remove_entry(self.bound_endpoint().unwrap().port);
            SOCKET_SET.remove(handle);
        }
    }
}
