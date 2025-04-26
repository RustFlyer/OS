use net::{NetPollState, udp::UdpSocket};
use systype::{SysError, SysResult};

use crate::processor::current_task;

use super::addr::SockAddr;

pub enum Sock {
    Udp(UdpSocket),
}

impl Sock {
    pub fn set_nonblocking(&self) {
        match self {
            Sock::Udp(udp) => udp.set_nonblocking(true),
        }
    }

    pub fn bind(&self, sockfd: usize, local_addr: SockAddr) -> SysResult<()> {
        match self {
            Sock::Udp(udp) => {
                let local_addr = local_addr.into_listen_endpoint();
                if let Some(prev_fd) = udp.check_bind(sockfd, local_addr) {
                    current_task()
                        .with_mut_fdtable(|table| table.dup3_with_flags(prev_fd, sockfd))?;
                    return Ok(());
                }
                udp.bind(local_addr)
            }
        }
    }

    pub fn listen(&self) -> SysResult<()> {
        match self {
            Sock::Udp(_udp) => Err(SysError::EOPNOTSUPP),
        }
    }

    pub async fn connect(&self, remote_addr: SockAddr) -> SysResult<()> {
        match self {
            Sock::Udp(udp) => {
                let remote_addr = remote_addr.into_endpoint();
                udp.connect(remote_addr)
            }
        }
    }

    pub fn peer_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Udp(udp) => {
                let peer_addr = SockAddr::from_endpoint(udp.peer_addr()?);
                Ok(peer_addr)
            }
        }
    }

    pub fn local_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Udp(udp) => {
                let local_addr = SockAddr::from_endpoint(udp.local_addr()?);
                Ok(local_addr)
            }
        }
    }
    pub async fn sendto(&self, buf: &[u8], remote_addr: Option<SockAddr>) -> SysResult<usize> {
        match self {
            Sock::Udp(udp) => match remote_addr {
                Some(addr) => udp.send_to(buf, addr.into_endpoint()).await,
                None => udp.send(buf).await,
            },
        }
    }
    pub async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        match self {
            Sock::Udp(udp) => {
                let (len, endpoint) = udp.recv_from(buf).await?;
                Ok((len, SockAddr::from_endpoint(endpoint)))
            }
        }
    }
    pub async fn poll(&self) -> NetPollState {
        match self {
            Sock::Udp(udp) => udp.poll().await,
        }
    }

    pub fn shutdown(&self, how: u8) -> SysResult<()> {
        match self {
            Sock::Udp(udp) => udp.shutdown(),
        }
    }
}
