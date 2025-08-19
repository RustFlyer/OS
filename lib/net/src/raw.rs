use crate::externf::__NetSocketIf_mod;
use core::{
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
    task::Waker,
};
use crate_interface::call_interface;
use smoltcp::{
    iface::SocketHandle,
    socket::raw::{self, Socket as RawSocketInner},
    wire::{IpProtocol, IpVersion, Ipv4Packet},
};

use mutex::SpinNoIrqLock;
use osfuture::{suspend_now, take_waker, yield_now};
use spin::RwLock;
use systype::error::{SysError, SysResult};

use crate::{
    NetPollState, SOCKET_SET, SocketSetWrapper,
    addr::{UNSPECIFIED_LISTEN_ENDPOINT, is_unspecified, to_endpoint},
    externf::NetSocketIf,
    tcp::has_signal,
};

/// RAW socket implementation for direct packet transmission
/// Allows sending/receiving raw IP packets bypassing TCP/UDP layers
pub struct RawSocket {
    handle: SocketHandle,
    protocol: IpProtocol,
    nonblock: AtomicBool,
    /// Whether to include IP header in user data
    hdr_included: AtomicBool,
}

impl RawSocket {
    /// Create a new raw socket with specified protocol
    ///
    /// # Arguments
    /// * `protocol` - IP protocol number (e.g., ICMP = 1, TCP = 6, UDP = 17)
    pub fn new(protocol: u8) -> Self {
        let ip_protocol = IpProtocol::from(protocol);
        let socket = SocketSetWrapper::new_raw_socket(ip_protocol, IpVersion::Ipv4);
        let handle = SOCKET_SET.add(socket);
        log::info!(
            "[RawSocket::new] add handle {}, protocol: {:?}",
            handle,
            ip_protocol
        );

        Self {
            handle,
            protocol: ip_protocol,
            nonblock: AtomicBool::new(false),
            hdr_included: AtomicBool::new(false),
        }
    }

    /// Create a new ICMP raw socket (commonly used for ping)
    pub fn new_icmp() -> Self {
        Self::new(1) // ICMP protocol number
    }

    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    pub fn set_hdr_included(&self, included: bool) {
        self.hdr_included.store(included, Ordering::Release);
    }

    pub fn is_hdr_included(&self) -> bool {
        self.hdr_included.load(Ordering::Acquire)
    }

    /// Send raw packet data
    ///
    /// # Arguments
    /// * `buf` - Raw packet data to send
    /// * `dst_addr` - Destination address (optional for some protocols)
    ///
    /// # Returns
    /// * `Ok(bytes_sent)` - Number of bytes successfully sent
    /// * `Err(error)` - Error if transmission failed
    pub async fn send_raw(
        &self,
        buf: &[u8],
        dst_addr: Option<smoltcp::wire::IpAddress>,
    ) -> SysResult<usize> {
        log::info!(
            "[RawSocket::send_raw] sending {} bytes, protocol: {:?}",
            buf.len(),
            self.protocol
        );

        let waker = take_waker().await;
        let bytes = self
            .block_on(|| {
                SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
                    if socket.can_send() {
                        // For raw sockets, we send the payload directly
                        socket.send_slice(buf).map_err(|e| {
                            log::warn!("[RawSocket::send_raw] send failed: {:?}", e);
                            SysError::EAGAIN
                        })?;
                        Ok(buf.len())
                    } else {
                        log::info!("[RawSocket::send_raw] can't send now, buffer full");
                        socket.register_send_waker(&waker);
                        Err(SysError::EAGAIN)
                    }
                })
            })
            .await?;

        log::info!("[RawSocket::send_raw] sent {} bytes", bytes);
        yield_now().await;
        Ok(bytes)
    }

    /// Send ICMP packet (for ping functionality)
    ///
    /// # Arguments
    /// * `icmp_type` - ICMP type (8 for echo request)
    /// * `icmp_code` - ICMP code (usually 0)
    /// * `identifier` - ICMP identifier
    /// * `sequence` - ICMP sequence number
    /// * `payload` - ICMP payload data
    pub async fn send_icmp(
        &self,
        icmp_type: u8,
        icmp_code: u8,
        identifier: u16,
        sequence: u16,
        payload: &[u8],
    ) -> SysResult<usize> {
        if self.protocol != IpProtocol::Icmp {
            return Err(SysError::EINVAL);
        }

        // Construct ICMP packet
        let icmp_len = 8 + payload.len(); // ICMP header (8 bytes) + payload
        let mut icmp_buf = alloc::vec![0u8; icmp_len];

        icmp_buf[0] = icmp_type;
        icmp_buf[1] = icmp_code;
        icmp_buf[2..4].copy_from_slice(&0u16.to_be_bytes()); // Checksum (will be calculated)
        icmp_buf[4..6].copy_from_slice(&identifier.to_be_bytes());
        icmp_buf[6..8].copy_from_slice(&sequence.to_be_bytes());
        icmp_buf[8..].copy_from_slice(payload);

        // Calculate ICMP checksum
        let checksum = Self::calculate_icmp_checksum(&icmp_buf);
        icmp_buf[2..4].copy_from_slice(&checksum.to_be_bytes());

        log::info!(
            "[RawSocket::send_icmp] sending ICMP type={}, code={}, id={}, seq={}",
            icmp_type,
            icmp_code,
            identifier,
            sequence
        );

        self.send_raw(&icmp_buf, None).await
    }

    /// Receive raw packet data
    ///
    /// # Arguments
    /// * `buf` - Buffer to store received data
    ///
    /// # Returns
    /// * `Ok(bytes_received)` - Number of bytes received
    /// * `Err(error)` - Error if reception failed
    pub async fn recv_raw(&self, buf: &mut [u8]) -> SysResult<usize> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok(len) => Ok(len),
            Err(_) => {
                log::warn!("[RawSocket::recv_raw] recv failed");
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    /// Peek at raw packet data without removing it from buffer
    pub async fn peek_raw(&self, buf: &mut [u8]) -> SysResult<usize> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok(len) => Ok(len),
            Err(_) => {
                log::warn!("[RawSocket::peek_raw] peek failed");
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    /// Private function for recv operations
    async fn recv_impl<F, T>(&self, mut op: F) -> SysResult<T>
    where
        F: FnMut(&mut raw::Socket) -> SysResult<T>,
    {
        let waker = take_waker().await;
        let ret = self
            .block_on(|| {
                SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
                    if socket.can_recv() {
                        op(socket)
                    } else {
                        log::info!("[RawSocket::recv_impl] no data available, registering waker");
                        socket.register_recv_waker(&waker);
                        Err(SysError::EAGAIN)
                    }
                })
            })
            .await;
        yield_now().await;
        ret
    }

    /// Receive raw packet and extract source IP from packet header
    ///
    /// # Arguments
    /// * `buf` - Buffer to store received data
    ///
    /// # Returns
    /// * `Ok((bytes_received, src_ip))` - Number of bytes received and source IP
    /// * `Err(error)` - Error if reception failed
    pub async fn recv_raw_with_addr(
        &self,
        buf: &mut [u8],
    ) -> SysResult<(usize, Option<smoltcp::wire::IpAddress>)> {
        let len = self.recv_raw(buf).await?;

        // Try to parse IP header to extract source address
        let src_addr = if len >= 20 && buf[0] >> 4 == 4 {
            // IPv4 packet
            if let Ok(ipv4_packet) = smoltcp::wire::Ipv4Packet::new_checked(&buf[..len]) {
                Some(smoltcp::wire::IpAddress::Ipv4(ipv4_packet.src_addr()))
            } else {
                None
            }
        } else if len >= 40 && buf[0] >> 4 == 6 {
            // IPv6 packet
            if let Ok(ipv6_packet) = smoltcp::wire::Ipv6Packet::new_checked(&buf[..len]) {
                Some(smoltcp::wire::IpAddress::Ipv6(ipv6_packet.src_addr()))
            } else {
                None
            }
        } else {
            None
        };

        Ok((len, src_addr))
    }

    /// Poll socket for readability/writability
    pub async fn poll(&self) -> NetPollState {
        let waker = take_waker().await;
        SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
            let readable = socket.can_recv();
            let writable = socket.can_send();

            if !readable {
                log::info!("[RawSocket::poll] not readable, register recv waker");
                socket.register_recv_waker(&waker);
            }
            if !writable {
                log::info!("[RawSocket::poll] not writable, register send waker");
                socket.register_send_waker(&waker);
            }

            NetPollState {
                readable,
                writable,
                hangup: false,
            }
        })
    }

    /// Close the raw socket
    pub fn shutdown(&self) -> SysResult<()> {
        SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
            log::info!("[RawSocket::shutdown] shutting down handle {}", self.handle);
        });
        let timestamp = SOCKET_SET.poll_interfaces();
        SOCKET_SET.check_poll(timestamp);
        Ok(())
    }

    /// Register waker for receive operations
    pub fn register_recv_waker(&self, waker: &Waker) {
        SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
            socket.register_recv_waker(waker);
        });
    }

    /// Register waker for send operations
    pub fn register_send_waker(&self, waker: &Waker) {
        SOCKET_SET.with_socket_mut::<raw::Socket, _, _>(self.handle, |socket| {
            socket.register_send_waker(waker);
        });
    }

    /// Async block_on helper (similar to UDP implementation)
    async fn block_on<F, T>(&self, mut f: F) -> SysResult<T>
    where
        F: FnMut() -> SysResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            loop {
                let timestamp = SOCKET_SET.poll_interfaces();
                let ret = f();
                SOCKET_SET.check_poll(timestamp);
                match ret {
                    Ok(t) => return Ok(t),
                    Err(SysError::EAGAIN) => {
                        suspend_now().await;
                        if has_signal() {
                            log::warn!("[RawSocket::block_on] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// Calculate ICMP checksum
    fn calculate_icmp_checksum(data: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        let mut i = 0;

        // Sum all 16-bit words
        while i < data.len() - 1 {
            sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
            i += 2;
        }

        // Add remaining byte if odd length
        if i < data.len() {
            sum += (data[i] as u32) << 8;
        }

        // Add carry bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        // One's complement
        !sum as u16
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        log::info!("[RawSocket::drop] removing handle {}", self.handle);

        SOCKET_SET.remove(self.handle);
        let timestamp = SOCKET_SET.poll_interfaces();
        SOCKET_SET.check_poll(timestamp);
    }
}
