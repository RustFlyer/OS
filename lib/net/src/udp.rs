use core::{
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
};

use mutex::SpinNoIrqLock;
use osfuture::{take_waker, yield_now};
use smoltcp::{
    iface::SocketHandle,
    socket::{
        self,
        udp::{self, BindError, SendError},
    },
    wire::{IpEndpoint, IpListenEndpoint},
};
use spin::RwLock;
use systype::{SysError, SysResult};

use crate::{
    NetPollState, SOCKET_SET, SocketSetWrapper,
    addr::{UNSPECIFIED_LISTEN_ENDPOINT, is_unspecified, to_endpoint},
    portmap::PORT_MAP,
};

const PORT_START: u16 = 0xc000;
const PORT_END: u16 = 0xffff;
static CURR: SpinNoIrqLock<u16> = SpinNoIrqLock::new(PORT_START);

pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpListenEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
    nonblock: AtomicBool,
}

impl UdpSocket {
    pub fn new() -> Self {
        let socket = SocketSetWrapper::new_udp_socket();
        let handle = SOCKET_SET.add(socket);
        Self {
            handle,
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblock: AtomicBool::new(false),
        }
    }

    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self.local_addr.try_read() {
            Some(addr) => addr.ok_or(SysError::ENOTCONN).map(to_endpoint),
            None => Err(SysError::ENOTCONN),
        }
    }

    pub fn peer_addr(&self) -> SysResult<IpEndpoint> {
        match self.peer_addr.try_read() {
            Some(addr) => addr.ok_or(SysError::ENOTCONN),
            None => Err(SysError::ENOTCONN),
        }
    }

    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }
}

impl UdpSocket {
    pub fn check_bind(&self, fd: usize, mut bound_addr: IpListenEndpoint) -> Option<usize> {
        if let Some((fd, prev_bound_addr)) = PORT_MAP.get(bound_addr.port) {
            if bound_addr == prev_bound_addr {
                return Some(fd);
            }
        }

        if bound_addr.port == 0 {
            bound_addr.port = UdpSocket::get_ephemeral_port();
        }
        PORT_MAP.insert(bound_addr.port, fd, bound_addr);
        None
    }

    pub fn bind(&self, mut bound_addr: IpListenEndpoint) -> SysResult<()> {
        let mut local_addr = self.local_addr.write();

        if bound_addr.port == 0 {
            bound_addr.port = UdpSocket::get_ephemeral_port();
        }

        if local_addr.is_some() {
            return Err(SysError::EINVAL);
        }

        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            socket.bind(bound_addr).map_err(|e| match e {
                BindError::InvalidState => SysError::EEXIST,
                BindError::Unaddressable => SysError::EINVAL,
            })
        })?;

        *local_addr = Some(bound_addr);

        Ok(())
    }

    pub async fn send_to(&self, buf: &[u8], remote_addr: IpEndpoint) -> SysResult<usize> {
        if remote_addr.port == 0 || remote_addr.addr.is_unspecified() {
            return Err(SysError::EINVAL);
        }

        if self.local_addr.read().is_none() {
            log::warn!(
                "[send_impl] UDP socket {}: not bound. Use 127.0.0.1",
                self.handle
            );
            self.bind(UNSPECIFIED_LISTEN_ENDPOINT)?;
        }

        let waker = take_waker().await;
        let bytes = SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            if socket.can_send() {
                socket.send_slice(buf, remote_addr).map_err(|e| match e {
                    SendError::BufferFull => {
                        log::warn!("socket send() failed, {e:?}");
                        socket.register_send_waker(&waker);
                        SysError::EAGAIN
                    }
                    SendError::Unaddressable => {
                        log::warn!("socket send() failed, {e:?}");
                        SysError::ECONNREFUSED
                    }
                })?;
                Ok(buf.len())
            } else {
                log::info!(
                    "[UdpSocket::send_impl] handle{} can't send now, tx buffer is full",
                    self.handle
                );
                socket.register_send_waker(&waker);
                Err(SysError::EAGAIN)
            }
        })?;
        log::info!("[UdpSocket::send_impl] send {bytes}bytes to {remote_addr:?}");
        yield_now().await;

        Ok(bytes)
    }

    pub async fn send(&self, buf: &[u8]) -> SysResult<usize> {
        let remote_addr = self.peer_addr()?;
        self.send_to(buf, remote_addr).await
    }

    pub async fn recv_impl<F, T>(&self, mut op: F) -> SysResult<T>
    where
        F: FnMut(&mut udp::Socket) -> SysResult<T>,
    {
        if self.local_addr.read().is_none() {
            log::warn!("socket send() failed");
            return Err(SysError::ENOTCONN);
        }
        let waker = take_waker().await;
        let ret = SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            if socket.can_recv() {
                // data available
                op(socket)
            } else if !socket.is_open() {
                log::warn!("UDP socket {}: recv() failed: not connected", self.handle);
                Err(SysError::ENOTCONN)
            } else {
                // no more data
                log::info!("[recv_impl] no more data, register waker and suspend now");
                socket.register_recv_waker(&waker);
                Err(SysError::EAGAIN)
            }
        });
        yield_now().await;
        ret
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> SysResult<(usize, IpEndpoint)> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint)),
            Err(e) => {
                log::warn!("[UdpSocket::recv_from] socket {} failed {e:?}", self.handle);
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    pub async fn recv(&self, buf: &mut [u8]) -> SysResult<usize> {
        let remote_endpoint = self.peer_addr()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket.recv_slice(buf).map_err(|_| {
                log::warn!("socket recv()  failed");
                SysError::EAGAIN
            })?;
            if !is_unspecified(remote_endpoint.addr) && remote_endpoint.addr != meta.endpoint.addr {
                return Err(SysError::EAGAIN);
            }
            if remote_endpoint.port != 0 && remote_endpoint.port != meta.endpoint.port {
                return Err(SysError::EAGAIN);
            }
            Ok(len)
        })
        .await
    }

    pub async fn peek_from(&self, buf: &mut [u8]) -> SysResult<(usize, IpEndpoint)> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint)),
            Err(_) => {
                log::warn!("socket recv_from() failed");
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    pub fn connect(&self, addr: IpEndpoint) -> SysResult<()> {
        if self.local_addr.read().is_none() {
            log::info!(
                "[UdpSocket::connect] don't have local addr, bind to UNSPECIFIED_LISTEN_ENDPOINT"
            );
            self.bind(UNSPECIFIED_LISTEN_ENDPOINT)?;
        }
        let mut self_peer_addr = self.peer_addr.write();
        *self_peer_addr = Some(addr);
        log::info!(
            "[UdpSocket::connect] handle {} local {} connected to remote {}",
            self.handle,
            self.local_addr.read().deref().unwrap(),
            addr
        );
        Ok(())
    }

    /// Close the socket.
    pub fn shutdown(&self) -> SysResult<()> {
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            log::warn!(
                "UDP socket {}: shutting down, remote {:?}",
                self.handle,
                self.peer_addr()
            );
            socket.close();
        });
        let timestamp = SOCKET_SET.poll_interfaces();
        SOCKET_SET.check_poll(timestamp);
        Ok(())
    }

    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> NetPollState {
        if self.local_addr.read().is_none() {
            return NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            };
        }
        let waker = take_waker().await;
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            let readable = socket.can_recv();
            let writable = socket.can_send();
            if !readable {
                log::info!("[UdpSocket::poll] not readable, register recv waker");
                socket.register_recv_waker(&waker);
            }
            if !writable {
                log::info!("[UdpSocket::poll] not writable, register send waker");
                socket.register_send_waker(&waker);
            }
            NetPollState {
                readable,
                writable,
                hangup: false,
            }
        })
    }

    pub fn get_ephemeral_port() -> u16 {
        let mut curr = CURR.lock();
        let port = *curr;
        *curr += 1;
        if *curr > PORT_END {
            *curr = PORT_START;
        }
        port
    }
}
