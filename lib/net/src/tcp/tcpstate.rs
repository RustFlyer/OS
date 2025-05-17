use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    task::Waker,
};

use alloc::{boxed::Box, vec::Vec};
use mutex::{SpinNoIrqLock, new_share_mutex};
use osfuture::{suspend_now, take_waker, yield_now};
use smoltcp::{
    iface::SocketHandle,
    socket::tcp::{self, ConnectError, State},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};
use systype::{SysError, SysResult};
use timer::sleep_ms;

use crate::{
    ETH0, NetPollState, SOCKET_SET, SocketSetWrapper,
    addr::{UNSPECIFIED_ENDPOINT_V4, UNSPECIFIED_IPV4, is_unspecified},
    tcp::LISTEN_TABLE,
};

use super::{
    RCV_SHUTDOWN, SEND_SHUTDOWN, SHUT_RD, SHUT_RDWR, SHUT_WR, SHUTDOWN_MASK, STATE_BUSY,
    STATE_CLOSED, STATE_CONNECTED, STATE_CONNECTING, STATE_LISTENING, core::TcpSocket, has_signal,
};

impl TcpSocket {
    /// Creates a new TCP socket.
    ///
    /// 此时并没有加到SocketSet中（还没有handle），在connect/listen中才会添加
    pub fn new_v4() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            shutdown: UnsafeCell::new(0),
            handle: UnsafeCell::new(None),
            local_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT_V4),
            peer_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT_V4),
            nonblock: AtomicBool::new(false),
            listen_handles: new_share_mutex(Vec::new()),
        }
    }

    /// Creates a new TCP socket that is already connected.
    fn new_connected(handle: SocketHandle, local_addr: IpEndpoint, peer_addr: IpEndpoint) -> Self {
        Self {
            state: AtomicU8::new(STATE_CONNECTED),
            shutdown: UnsafeCell::new(0),
            handle: UnsafeCell::new(Some(handle)),
            local_addr: UnsafeCell::new(local_addr),
            peer_addr: UnsafeCell::new(peer_addr),
            nonblock: AtomicBool::new(false),
            listen_handles: new_share_mutex(Vec::new()),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self.get_state() {
            state
                if (state == STATE_CONNECTED)
                    | (state == STATE_LISTENING)
                    | (state == STATE_CLOSED) =>
            {
                Ok(unsafe { self.local_addr.get().read() })
            }
            _ => Err(SysError::ENOTCONN),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn peer_addr(&self) -> SysResult<IpEndpoint> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING => Ok(unsafe { self.peer_addr.get().read() }),
            _ => Err(SysError::ENOTCONN),
        }
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// This will result in `read`, `write`, `recv` and `send` operations
    /// becoming nonblocking, i.e., immediately returning from their calls.
    /// If the IO operation is successful, `Ok` is returned and no further
    /// action is required. If the IO operation could not be completed and needs
    /// to be retried, an error with kind
    /// [`Err(WouldBlock)`](AxError::WouldBlock) is returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Connects to the given address and port.
    ///
    /// The local port is generated automatically.
    pub async fn connect(&self, remote_addr: IpEndpoint) -> SysResult<()> {
        yield_now().await;
        // 将STATE_CLOSED改为STATE_CONNECTING，在poll_connect的时候，
        // 会再变为STATE_CONNECTED
        self.update_state(STATE_CLOSED, STATE_CONNECTING, || {
            // SAFETY: no other threads can read or write these fields.
            let handle = unsafe { self.handle.get().read() }
                .unwrap_or_else(|| SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()));

            // TODO: check remote addr unreachable
            let bound_endpoint = self.bound_endpoint()?;
            let iface = &ETH0.get().unwrap().iface;
            let (local_endpoint, remote_endpoint) = SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    socket
                        .connect(iface.lock().context(), remote_addr, bound_endpoint)
                        .or_else(|e| match e {
                            // When attempting to perform an operation, the socket is in an
                            // invalid state. Such as attempting to call the connection operation
                            // again on an already connected socket, or performing
                            // the operation on a closed socket
                            ConnectError::InvalidState => {
                                log::warn!("[TcpSocket::connect] failed: InvalidState");
                                Err(SysError::EBADF)
                            }
                            // The target address or port attempting to connect is unreachable
                            ConnectError::Unaddressable => {
                                log::warn!("[TcpSocket::connect] failed: Unaddressable");
                                Err(SysError::EADDRNOTAVAIL)
                            }
                        })?;
                    Ok((
                        socket.local_endpoint().unwrap(),
                        socket.remote_endpoint().unwrap(),
                    ))
                })?;
            unsafe {
                // SAFETY: no other threads can read or write these fields as we
                // have changed the state to `BUSY`.
                self.local_addr.get().write(local_endpoint);
                self.peer_addr.get().write(remote_endpoint);
                self.handle.get().write(Some(handle));
            }
            Ok(())
        })
        .unwrap_or_else(|_| {
            log::warn!("[TcpSocket::connect] failed: already connected");
            Err(SysError::EEXIST)
        })?; // EISCONN

        // Here our state must be `CONNECTING`, and only one thread can run here.
        if self.is_nonblocking() {
            Err(SysError::EINPROGRESS)
        } else {
            self.block_on_async(|| async {
                let NetPollState { writable, .. } = self.poll_connect().await;
                if !writable {
                    log::warn!("[TcpSocket::connect] failed: try again");
                    Err(SysError::EAGAIN)
                } else if self.get_state() == STATE_CONNECTED {
                    log::warn!("[TcpSocket::connect] connect to {:?} success", remote_addr);
                    Ok(())
                } else {
                    log::warn!("[TcpSocket::connect] failed, connection refused");
                    Err(SysError::ECONNREFUSED)
                }
            })
            .await
        }
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// If the given port is 0, it generates one automatically.
    ///
    /// It's must be called before [`listen`](Self::listen) and
    /// [`accept`](Self::accept).
    pub fn bind(&self, mut local_addr: IpEndpoint) -> SysResult<()> {
        self.update_state(STATE_CLOSED, STATE_CLOSED, || {
            // TODO: check addr is available
            if local_addr.port == 0 {
                let port = get_ephemeral_port()?;
                local_addr.port = port;
                log::info!("[TcpSocket::bind] local port is 0, use port {port}");
            }

            unsafe {
                let old = self.local_addr.get().read();
                if old != UNSPECIFIED_ENDPOINT_V4 {
                    log::warn!("socket bind() failed: {:?} already bound", local_addr);
                    return Err(SysError::EINVAL);
                }
                // FIXME
                if let IpAddress::Ipv6(v6) = local_addr.addr {
                    if v6.is_unspecified() {
                        log::warn!("[TcpSocket::bind] Unstable: just use ipv4 instead of ipv6 when ipv6 is unspecified");
                        local_addr.addr = UNSPECIFIED_IPV4;
                    }
                }
                self.local_addr.get().write(local_addr);
            }
            Ok(())
        })
        .unwrap_or_else(|_| {
            log::warn!("socket bind() failed: {:?} already bound", local_addr);
            Err(SysError::EINVAL)
        })
    }

    /// Starts listening on the bound address and port.
    ///
    /// It's must be called after [`bind`](Self::bind) and before
    /// [`accept`](Self::accept).
    pub fn listen(&self, waker: &Waker) -> SysResult<()> {
        self.update_state(STATE_CLOSED, STATE_LISTENING, || {
            let bound_endpoint = self.bound_endpoint()?;
            unsafe {
                (*self.local_addr.get()).port = bound_endpoint.port;
            }
            LISTEN_TABLE.listen(bound_endpoint, waker, self.listen_handles.clone())?;

            // log::info!("[TcpSocket::listen] listening on {bound_endpoint:?}");
            for _ in 0..32 {
                let sock_handle = SOCKET_SET.add(SocketSetWrapper::new_tcp_socket());
                SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(sock_handle, |sock| {
                    sock.listen(bound_endpoint).unwrap();
                });
                self.listen_handles.lock().push(sock_handle);
            }

            Ok(())
        })
        .unwrap_or(Ok(())) // ignore simultaneous `listen`s.
    }

    /// Accepts a new connection.
    ///
    /// This function will block the calling thread until a new TCP connection
    /// is established. When established, a new [`TcpSocket`] is returned.
    ///
    /// It's must be called after [`bind`](Self::bind) and
    /// [`listen`](Self::listen).
    pub async fn accept(&self) -> SysResult<TcpSocket> {
        if !self.is_listening() {
            log::warn!("socket accept() failed: not listen");
            return Err(SysError::EINVAL);
        }

        // let waker = take_waker().await;
        // SAFETY: `self.local_addr` should be initialized after `bind()`.

        // self.block_on(|| {
        //     let mut listen_handles = self.listen_handles.lock();
        //     for (i, &handle) in listen_handles.iter().enumerate() {
        //         let established = SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |sock| {
        //             log::debug!("has established? {:?}", sock.state());
        //             if sock.state() == State::Established {
        //                 let peer_addr = sock.remote_endpoint().unwrap();
        //                 let local_addr = sock.local_endpoint().unwrap();

        //                 Some((handle, local_addr, peer_addr))
        //             } else {
        //                 // sock.register_recv_waker(&waker);
        //                 None
        //             }
        //         });

        //         if let Some((hdl, local, peer)) = established {
        //             let handle = listen_handles.remove(i);
        //             let new_handle = SOCKET_SET.add(SocketSetWrapper::new_tcp_socket());
        //             SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(new_handle, |sock| {
        //                 sock.listen(local).unwrap();
        //             });
        //             listen_handles.push(new_handle);
        //             unsafe {
        //                 self.handle.get().write(Some(handle));
        //             }

        //             return Ok(TcpSocket::new_connected(hdl, local, peer));
        //         }
        //     }

        //     Err(SysError::EAGAIN)
        // })
        // .await
        let local_port = unsafe { self.local_addr.get().read().port };
        self.block_on(|| {
            let (handle, (local_addr, peer_addr)) = LISTEN_TABLE.accept(local_port)?;
            log::info!("TCP socket accepted a new connection {}", peer_addr);
            Ok(TcpSocket::new_connected(handle, local_addr, peer_addr))
        })
        .await
    }

    /// Close the connection.
    pub fn shutdown(&self, how: u8) -> SysResult<()> {
        // SAFETY: shutdown won't be called in multiple threads
        unsafe {
            let shutdown = self.shutdown.get();
            match how {
                SHUT_RD => *shutdown |= RCV_SHUTDOWN,
                SHUT_WR => *shutdown |= SEND_SHUTDOWN,
                SHUT_RDWR => *shutdown |= SHUTDOWN_MASK,
                _ => return Err(SysError::EINVAL),
            }
        }

        // stream
        self.update_state(STATE_CONNECTED, STATE_CLOSED, || {
            // SAFETY: `self.handle` should be initialized in a connected socket, and
            // no other threads can read or write it.
            let handle = unsafe { self.handle.get().read().unwrap() };
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                log::warn!(
                    "TCP handle {handle}: shutting down, before state is {:?}",
                    socket.state()
                );
                socket.close();
                log::warn!(
                    "TCP handle {handle}: shutting down, after state is {:?}",
                    socket.state()
                );
            });
            // unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT) }; // clear bound
            // address
            let timestamp = SOCKET_SET.poll_interfaces();
            SOCKET_SET.check_poll(timestamp);
            Ok(())
        })
        .unwrap_or(Ok(()))?;

        // listener
        self.update_state(STATE_LISTENING, STATE_CLOSED, || {
            // SAFETY: `self.local_addr` should be initialized in a listening socket,
            // and no other threads can read or write it.
            let local_port = unsafe { self.local_addr.get().read().port };
            unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT_V4) }; // clear bound address
            LISTEN_TABLE.unlisten(local_port);
            let timestamp = SOCKET_SET.poll_interfaces();
            SOCKET_SET.check_poll(timestamp);
            Ok(())
        })
        .unwrap_or(Ok(()))?;
        // ignore for other states
        Ok(())
    }

    pub fn register_recv_waker(&self, waker: &Waker) {
        let handle = unsafe { self.handle.get().read().unwrap() };
        SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            socket.register_recv_waker(waker);
        });
    }

    pub fn register_send_waker(&self, waker: &Waker) {
        let handle = unsafe { self.handle.get().read().unwrap() };
        SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            socket.register_send_waker(waker);
        });
    }
}

/// Private methods
impl TcpSocket {
    #[inline]
    pub(crate) fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    #[inline]
    pub(crate) fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    /// Update the state of the socket atomically.
    ///
    /// If the current state is `expect`, it first changes the state to
    /// `STATE_BUSY`, then calls the given function. If the function returns
    /// `Ok`, it changes the state to `new`, otherwise it changes the state
    /// back to `expect`.
    ///
    /// It returns `Ok` if the current state is `expect`, otherwise it returns
    /// the current state in `Err`.
    fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<SysResult<T>, u8>
    where
        F: FnOnce() -> SysResult<T>,
    {
        match self
            .state
            .compare_exchange(expect, STATE_BUSY, Ordering::Acquire, Ordering::Acquire)
        {
            Ok(_) => {
                let res = f();
                if res.is_ok() {
                    self.set_state(new);
                } else {
                    self.set_state(expect);
                }
                Ok(res)
            }
            Err(old) => Err(old),
        }
    }

    #[inline]
    pub(crate) fn is_connecting(&self) -> bool {
        self.get_state() == STATE_CONNECTING
    }

    #[inline]
    pub(crate) fn is_connected(&self) -> bool {
        self.get_state() == STATE_CONNECTED
    }

    #[inline]
    pub(crate) fn is_listening(&self) -> bool {
        self.get_state() == STATE_LISTENING
    }

    /// 构建并返回当前对象绑定的网络端点信息。
    /// 具体来说，它从对象的 local_addr
    /// 属性中读取IP地址和端口信息，如果端口未指定则分配一个临时端口，
    /// 并确保返回一个有效的端点（IpListenEndpoint）。
    pub(crate) fn bound_endpoint(&self) -> SysResult<IpListenEndpoint> {
        // SAFETY: no other threads can read or write `self.local_addr`.
        let local_addr = unsafe { self.local_addr.get().read() };
        let port = if local_addr.port != 0 {
            local_addr.port
        } else {
            get_ephemeral_port()?
        };
        assert_ne!(port, 0);
        let addr = if !is_unspecified(local_addr.addr) {
            Some(local_addr.addr)
        } else {
            None
        };
        Ok(IpListenEndpoint { addr, port })
    }

    /// Block the current thread until the given function completes or fails.
    ///
    /// If the socket is non-blocking, it calls the function once and returns
    /// immediately. Otherwise, it may call the function multiple times if it
    /// returns [`Err(WouldBlock)`](AxError::WouldBlock).
    pub(crate) async fn block_on<F, T>(&self, mut f: F) -> SysResult<T>
    where
        F: FnMut() -> SysResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            log::debug!("[block_on] socket is blocking");
            loop {
                let timestamp = SOCKET_SET.poll_interfaces();
                let ret = f();
                SOCKET_SET.check_poll(timestamp);
                match ret {
                    Ok(t) => {
                        log::warn!("[block_on] get out");
                        return Ok(t);
                    }
                    Err(SysError::EAGAIN) => {
                        // log::warn!("[block_on] this thread is suspended");
                        suspend_now().await;
                        // sleep_ms(5).await;
                        if has_signal() {
                            log::warn!("[TcpSocket::block_on] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    pub(crate) async fn block_on_async<F, T, Fut>(&self, mut f: F) -> SysResult<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = SysResult<T>>,
    {
        if self.is_nonblocking() {
            f().await
        } else {
            loop {
                let timestamp = SOCKET_SET.poll_interfaces();
                let ret = f().await;
                SOCKET_SET.check_poll(timestamp);
                match ret {
                    Ok(t) => return Ok(t),
                    Err(SysError::EAGAIN) => {
                        suspend_now().await;
                        if has_signal() {
                            log::warn!("[TcpSocket::block_on_async] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

fn get_ephemeral_port() -> SysResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: SpinNoIrqLock<u16> = SpinNoIrqLock::new(PORT_START);

    let mut curr = CURR.lock();
    let mut tries = 0;
    while tries <= PORT_END - PORT_START {
        let port = *curr;
        if *curr == PORT_END {
            *curr = PORT_START;
        } else {
            *curr += 1;
        }
        if LISTEN_TABLE.can_listen(port) {
            return Ok(port);
        }
        tries += 1;
    }
    log::warn!("no avaliable ports!");
    Err(SysError::EADDRINUSE)
}
