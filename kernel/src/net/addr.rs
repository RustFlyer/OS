//! `SockAddr` is a C language structure layout, used for system calls to
//! interact with users. It is ** network byte order  (big endian) **
//!
//! `IpEndpoint` is host byte order

use core::net::{Ipv4Addr, Ipv6Addr};

use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint};
use systype::SysError;

#[derive(Clone, Copy)]
#[repr(C)]
/// IPv4 address
pub struct SockAddrIn {
    /// always set to `AF_INET`
    pub family: u16,
    /// port in network byte order
    pub port: [u8; 2],
    /// address in network byte order
    pub addr: [u8; 4],
    pub zero: [u8; 8],
}

#[derive(Clone, Copy)]
#[repr(C)]
/// IPv6 address
pub struct SockAddrIn6 {
    pub family: u16,
    /// port in network byte order (big endian)
    pub port: [u8; 2],
    pub flowinfo: u32,
    pub addr: [u8; 16],
    pub scope: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
/// Unix domain socket address
pub struct SockAddrUn {
    pub family: u16,
    pub path: [u8; 108],
}

/// socket address family
#[allow(non_camel_case_types)]
pub enum SaFamily {
    AF_UNIX = 1,
    /// ipv4
    AF_INET = 2,
    /// ipv6
    AF_INET6 = 10,
}

impl TryFrom<u16> for SaFamily {
    type Error = SysError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::AF_UNIX),
            2 => Ok(Self::AF_INET),
            10 => Ok(Self::AF_INET6),
            _ => Err(Self::Error::EINVAL),
        }
    }
}

impl From<SaFamily> for u16 {
    fn from(value: SaFamily) -> Self {
        match value {
            SaFamily::AF_UNIX => 1,
            SaFamily::AF_INET => 2,
            SaFamily::AF_INET6 => 10,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
/// `SockAddr` is a superset of `SocketAddr` in `core::net` since it also
/// includes the address for socket communication between Unix processes. And it
/// is a user oriented program with a C language structure layout, used for
/// system calls to interact with users
pub union SockAddr {
    pub family: u16,
    pub ipv4: SockAddrIn,
    pub ipv6: SockAddrIn6,
    pub unix: SockAddrUn,
}

impl SockAddr {
    /// You should make sure that `SockAddr` is IpEndpoint
    pub fn into_endpoint(&self) -> IpEndpoint {
        unsafe {
            match SaFamily::try_from(self.family).unwrap() {
                SaFamily::AF_INET => IpEndpoint::new(
                    IpAddress::Ipv4(Ipv4Addr::from(self.ipv4.addr)),
                    u16::from_be_bytes(self.ipv4.port),
                ),
                SaFamily::AF_INET6 => IpEndpoint::new(
                    IpAddress::Ipv6(Ipv6Addr::from(self.ipv6.addr)),
                    u16::from_be_bytes(self.ipv6.port),
                ),
                SaFamily::AF_UNIX => panic!("Shouldn't get there"),
            }
        }
    }

    pub fn into_listen_endpoint(&self) -> IpListenEndpoint {
        unsafe {
            match SaFamily::try_from(self.family).unwrap() {
                SaFamily::AF_INET => self.ipv4.into(),
                SaFamily::AF_INET6 => self.ipv6.into(),
                SaFamily::AF_UNIX => panic!("Shouldn't get there"),
            }
        }
    }

    pub fn from_endpoint(endpoint: IpEndpoint) -> Self {
        match endpoint.addr {
            IpAddress::Ipv4(v4) => Self {
                ipv4: endpoint.into(),
            },
            IpAddress::Ipv6(v6) => Self {
                ipv6: endpoint.into(),
            },
        }
    }
}

impl From<SockAddrIn> for IpEndpoint {
    fn from(v4: SockAddrIn) -> Self {
        IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Addr::from(v4.addr)),
            u16::from_be_bytes(v4.port),
        )
    }
}

impl From<SockAddrIn6> for IpEndpoint {
    fn from(v6: SockAddrIn6) -> Self {
        IpEndpoint::new(
            IpAddress::Ipv6(Ipv6Addr::from(v6.addr)),
            u16::from_be_bytes(v6.port),
        )
    }
}

impl From<IpEndpoint> for SockAddrIn {
    fn from(v4: IpEndpoint) -> Self {
        if let IpAddress::Ipv4(v4_addr) = v4.addr {
            return Self {
                family: SaFamily::AF_INET.into(),
                port: v4.port.to_be_bytes(),
                addr: unsafe { core::mem::transmute::<Ipv4Addr, [u8; 4]>(v4_addr) },
                zero: [0; 8],
            };
        } else {
            // this won't happen
            panic!();
        }
    }
}

impl From<IpEndpoint> for SockAddrIn6 {
    fn from(v6: IpEndpoint) -> Self {
        if let IpAddress::Ipv6(v6_addr) = v6.addr {
            return Self {
                family: SaFamily::AF_INET6.into(),
                port: v6.port.to_be_bytes(),
                flowinfo: 0,
                addr: unsafe { core::mem::transmute::<Ipv6Addr, [u8; 16]>(v6_addr) },
                scope: 0,
            };
        } else {
            panic!();
        }
    }
}

impl From<SockAddrIn> for IpListenEndpoint {
    fn from(v4: SockAddrIn) -> Self {
        let addr = Ipv4Addr::from(v4.addr);
        let addr = if addr.is_unspecified() {
            None
        } else {
            Some(IpAddress::Ipv4(addr))
        };
        Self {
            addr,
            port: u16::from_be_bytes(v4.port),
        }
    }
}

impl From<SockAddrIn6> for IpListenEndpoint {
    fn from(v6: SockAddrIn6) -> Self {
        let addr = Ipv6Addr::from(v6.addr);
        let addr = if addr.is_unspecified() {
            None
        } else {
            Some(IpAddress::Ipv6(addr))
        };
        Self {
            addr,
            port: u16::from_be_bytes(v6.port),
        }
    }
}
