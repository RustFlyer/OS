use core::task::Waker;

use alloc::collections::vec_deque::VecDeque;
use smoltcp::{
    iface::SocketHandle,
    wire::{IpAddress, IpListenEndpoint, IpVersion},
};

use crate::{SOCKET_SET, socketset::LISTEN_QUEUE_SIZE};

pub const PORT_NUM: usize = 65536;

/// An entry in the listen table, representing a specific listening endpoint.
///
/// This struct holds the information related to a specific listening IP address
/// and port. It also manages the SYN queue and the waker for handling incoming
/// TCP connections.
#[derive(Debug)]
pub struct ListenTableEntry {
    /// The IP address and port being listened on.
    pub(crate) listen_endpoint: IpListenEndpoint,
    /// The SYN queue holding incoming TCP connection handles.
    pub(crate) syn_queue: VecDeque<SocketHandle>,
    /// The waker used to wake up the listening socket when a new connection
    /// arrives.
    pub(crate) waker: Waker,
}

impl ListenTableEntry {
    pub fn new(listen_endpoint: IpListenEndpoint, waker: &Waker) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
            waker: waker.clone(),
        }
    }

    #[inline]
    /// Linux内核有一个特殊的机制，叫做 IPv4-mapped IPv6
    /// addresses，允许IPv6套接字接收IPv4连接
    ///
    /// 1. 当IPv6套接字绑定到::（全0地址）时，
    ///    内核会允许该套接字接受任何传入的连接，无论其是IPv4还是IPv6地址。
    /// 2. 对于从IPv4地址到来的连接，内核会将其转换为IPv4-mapped
    ///    IPv6地址，即::ffff:a.b.c.d格式，其中a.b.c.d是IPv4地址。
    pub(crate) fn can_accept(&self, dst: IpAddress) -> bool {
        match self.listen_endpoint.addr {
            Some(addr) => {
                if addr == dst {
                    return true;
                }
                if let IpAddress::Ipv6(v6) = addr {
                    if v6.is_unspecified() {
                        return true;
                    }

                    match dst {
                        IpAddress::Ipv4(v4) => {
                            if dst.version() == IpVersion::Ipv4
                                && v6.is_ipv4_mapped()
                                && v6.as_octets()[12..] == v4.as_octets()[..]
                            {
                                return true;
                            }
                        }
                        _ => (),
                    }
                }

                false
            }
            None => true,
        }
    }

    pub fn wake(self) {
        self.waker.wake_by_ref()
    }
}

impl Drop for ListenTableEntry {
    fn drop(&mut self) {
        for &handle in &self.syn_queue {
            SOCKET_SET.remove(handle);
        }
    }
}
