use core::{sync::atomic::Ordering, task::Waker};

use alloc::sync::Arc;
use net::{
    NetPollState,
    addr::UNSPECIFIED_IPV4,
    raw::RawSocket,
    tcp::core::TcpSocket,
    udp::UdpSocket,
    unix::{UnixSocket, extract_path_from_sockaddr_un},
};
use smoltcp::wire::IpEndpoint;
use systype::error::{SysError, SysResult};

use crate::processor::current_task;

use super::{
    addr::{SaFamily, SockAddr, SockAddrUn},
    check_unix_path, is_local_ip,
};

pub enum Sock {
    Tcp(TcpSocket),
    Udp(UdpSocket),
    Unix(Arc<UnixSocket>),
    Raw(RawSocket),
}

impl Sock {
    pub fn set_nonblocking(&self) {
        match self {
            Sock::Tcp(tcp) => tcp.set_nonblocking(true),
            Sock::Udp(udp) => udp.set_nonblocking(true),
            Sock::Unix(_unix) => (),
            Sock::Raw(raw) => raw.set_nonblocking(true),
        }
    }

    pub fn bind(&self, sockfd: usize, local_addr: SockAddr) -> SysResult<()> {
        let family = SaFamily::try_from(unsafe { local_addr.family })?;
        match (self, family) {
            (Sock::Tcp(tcp), SaFamily::AF_INET) | (Sock::Tcp(tcp), SaFamily::AF_INET6) => {
                let listen_ep = local_addr.as_listen_endpoint().ok_or(SysError::EINVAL)?;
                if !is_local_ip(&listen_ep) {
                    return Err(SysError::EADDRNOTAVAIL);
                }
                let addr = listen_ep.addr.unwrap_or(UNSPECIFIED_IPV4);
                tcp.bind(IpEndpoint::new(addr, listen_ep.port))
            }
            (Sock::Udp(udp), SaFamily::AF_INET) | (Sock::Udp(udp), SaFamily::AF_INET6) => {
                let listen_ep = local_addr.as_listen_endpoint().ok_or(SysError::EINVAL)?;
                if !is_local_ip(&listen_ep) {
                    return Err(SysError::EADDRNOTAVAIL);
                }
                if let Some(prev_fd) = udp.check_bind(sockfd, listen_ep) {
                    current_task()
                        .with_mut_fdtable(|table| table.dup3_with_flags(prev_fd, sockfd))?;
                    return Ok(());
                }
                udp.bind(listen_ep)
            }
            (Sock::Raw(_raw), SaFamily::AF_INET) | (Sock::Raw(_raw), SaFamily::AF_INET6) => {
                // RAW sockets typically don't need to bind to specific addresses
                // They operate at the IP layer
                Ok(())
            }
            (Sock::Unix(unix), SaFamily::AF_UNIX) => {
                let path = local_addr.as_unix_path().ok_or(SysError::EINVAL)?;
                match check_unix_path(&path) {
                    Ok(()) => unix.clone().bind(&path),
                    Err(e) => Err(e),
                }
            }
            _ => Err(SysError::EAFNOSUPPORT),
        }
    }

    pub fn listen(&self) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.listen(current_task().waker_mut().as_ref().unwrap()),
            Sock::Udp(_udp) => Err(SysError::EOPNOTSUPP),
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(_unix) => Err(SysError::EOPNOTSUPP),
        }
    }

    pub async fn connect(&self, remote_addr: SockAddr) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => {
                let remote_addr = remote_addr.as_endpoint();
                tcp.connect(remote_addr).await
            }
            Sock::Udp(udp) => {
                let remote_addr = remote_addr.as_endpoint();
                udp.connect(remote_addr)
            }
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(unix) => unsafe {
                let path = extract_path_from_sockaddr_un(&remote_addr.unix.path);
                unix.connect(&path)
            },
        }
    }

    pub fn peer_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Tcp(tcp) => {
                let peer_addr = SockAddr::from_endpoint(tcp.peer_addr()?);
                Ok(peer_addr)
            }
            Sock::Udp(udp) => {
                let peer_addr = SockAddr::from_endpoint(udp.peer_addr()?);
                Ok(peer_addr)
            }
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(unix) => {
                let peer = unix.peer.lock();
                if let Some(peer_sock) = &*peer {
                    let path_opt = peer_sock.path.lock();
                    if let Some(path) = &*path_opt {
                        let mut addr = SockAddrUn {
                            family: SaFamily::AF_UNIX as u16,
                            path: [0; 108],
                        };
                        let bytes = path.as_bytes();
                        let len = bytes.len().min(108);
                        addr.path[..len].copy_from_slice(&bytes[..len]);
                        Ok(SockAddr { unix: addr })
                    } else {
                        Err(SysError::ENOTCONN)
                    }
                } else {
                    Err(SysError::ENOTCONN)
                }
            }
        }
    }

    pub fn local_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Tcp(tcp) => {
                let local_addr = SockAddr::from_endpoint(tcp.local_addr()?);
                Ok(local_addr)
            }
            Sock::Udp(udp) => {
                let local_addr = SockAddr::from_endpoint(udp.local_addr()?);
                Ok(local_addr)
            }
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(unix) => {
                let path_opt = unix.path.lock();
                if let Some(path) = &*path_opt {
                    let mut addr = SockAddrUn {
                        family: SaFamily::AF_UNIX as u16,
                        path: [0; 108],
                    };
                    let bytes = path.as_bytes();
                    let len = bytes.len().min(108);
                    addr.path[..len].copy_from_slice(&bytes[..len]);
                    Ok(SockAddr { unix: addr })
                } else {
                    Err(SysError::ENOTCONN)
                }
            }
        }
    }

    pub async fn sendto(&self, buf: &[u8], remote_addr: Option<SockAddr>) -> SysResult<usize> {
        match self {
            Sock::Tcp(tcp) => tcp.send(buf).await,
            Sock::Udp(udp) => match remote_addr {
                Some(addr) => udp.send_to(buf, addr.as_endpoint()).await,
                None => udp.send(buf).await,
            },
            Sock::Raw(raw) => {
                let dst_addr = remote_addr.map(|addr| addr.as_endpoint().addr);
                raw.send_raw(buf, dst_addr).await
            }
            Sock::Unix(unix) => unix.send(buf),
        }
    }

    pub async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        match self {
            Sock::Tcp(tcp) => {
                let bytes = tcp.recv(buf).await?;
                Ok((bytes, SockAddr::from_endpoint(tcp.peer_addr()?)))
            }
            Sock::Udp(udp) => {
                let (len, endpoint) = udp.recv_from(buf).await?;
                Ok((len, SockAddr::from_endpoint(endpoint)))
            }
            Sock::Raw(raw) => {
                let (len, src_addr_opt) = raw.recv_raw_with_addr(buf).await?;
                let endpoint = if let Some(src_addr) = src_addr_opt {
                    smoltcp::wire::IpEndpoint::new(src_addr, 0)
                } else {
                    smoltcp::wire::IpEndpoint::new(smoltcp::wire::IpAddress::v4(0, 0, 0, 0), 0)
                };
                Ok((len, SockAddr::from_endpoint(endpoint)))
            }
            Sock::Unix(unix) => {
                let n = unix.recv(buf)?;
                let mut addr = SockAddrUn {
                    family: 1,
                    path: [0; 108],
                };
                if let Some(path) = &*unix.path.lock() {
                    let bytes = path.as_bytes();
                    addr.path[..bytes.len()].copy_from_slice(bytes);
                }
                Ok((n, SockAddr { unix: addr }))
            }
        }
    }

    pub async fn poll(&self) -> NetPollState {
        match self {
            Sock::Tcp(tcp) => tcp.poll().await,
            Sock::Udp(udp) => udp.poll().await,
            Sock::Raw(raw) => raw.poll().await,
            Sock::Unix(_unix) => unimplemented!(),
        }
    }

    pub fn shutdown(&self, how: u8) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.shutdown(how),
            Sock::Udp(udp) => udp.shutdown(),
            Sock::Raw(raw) => raw.shutdown(),
            Sock::Unix(_unix) => unimplemented!(),
        }
    }

    pub async fn accept(&self) -> SysResult<TcpSocket> {
        match self {
            Sock::Tcp(tcp) => {
                let new_tcp = tcp.accept().await?;
                Ok(new_tcp)
            }
            Sock::Udp(_udp) => Err(SysError::EOPNOTSUPP),
            Sock::Raw(_raw) => Err(SysError::EOPNOTSUPP),
            Sock::Unix(_unix) => unimplemented!(),
        }
    }

    pub fn _register_recv_waker(&self, waker: Waker) {
        match self {
            Sock::Tcp(tcp) => tcp.register_recv_waker(&waker),
            Sock::Udp(udp) => udp.register_recv_waker(&waker),
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(_unix) => unimplemented!(),
        }
    }

    pub fn _register_send_waker(&self, waker: Waker) {
        match self {
            Sock::Tcp(tcp) => tcp.register_send_waker(&waker),
            Sock::Udp(udp) => udp.register_send_waker(&waker),
            Sock::Raw(_raw) => {
                todo!()
            }
            Sock::Unix(_unix) => unimplemented!(),
        }
    }

    pub fn set_reuse_addr(&self, val: bool) {
        if let Sock::Udp(udp) = self {
            udp.reuse_addr.store(val, Ordering::SeqCst);
        }
    }

    pub fn set_reuse_port(&self, val: bool) {
        if let Sock::Udp(udp) = self {
            udp.reuse_port.store(val, Ordering::SeqCst);
        }
    }
}
