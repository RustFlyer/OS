use core::net::Ipv6Addr;

use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint};

/// checks whether a ip address is unspecified, which means that
/// ip is filled with 0.
///
/// - ipv4 == \[0, 0, 0, 0\]
/// - ipv6 == \[0, 0, 0, 0, 0, 0\]
pub fn is_unspecified(ip: IpAddress) -> bool {
    ip.is_unspecified()
}

/// `IpListenEndpoint` is an internet endpoint address for listening.
/// In contrast with `Endpoint`, `ListenEndpoint` allows not specifying the address,
/// in order to listen on a given port at all our addresses.
///
/// An endpoint can be constructed from a port, in which case the address is unspecified.
///
/// this function can convert `IpListenEndpoint` to `IpEndpoint`. In fact, it just unwrap
/// the option of `addr` in `IpListenEndpoint`.
///
/// #Tips
/// - `IpListenEndpoint` is mostly used in situations with multiple network cards
///   (multiple ip addresses) when its addr is None.
pub fn to_endpoint(listen_endpoint: IpListenEndpoint) -> IpEndpoint {
    let ip = match listen_endpoint.addr {
        Some(ip) => ip,
        None => UNSPECIFIED_IPV4,
    };
    IpEndpoint::new(ip, listen_endpoint.port)
}

/// when you do not specify an ipv4 address for IpListenEndpoint, [`to_endpoint`] will
/// return [`UNSPECIFIED_IPV4`] as IpEndpoint's addr.
///
/// it can be used in uninitialized ipv4 address variable, too.
pub const UNSPECIFIED_IPV4: IpAddress = IpAddress::v4(0, 0, 0, 0);

/// `UNSPECIFIED_ENDPOINT_V4` can be used in uninitialized ipv4 endpoint address variable.
pub const UNSPECIFIED_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IPV4, 0);

/// `UNSPECIFIED_LISTEN_ENDPOINT` can be used in uninitialized listen endpoint address variable.
pub const UNSPECIFIED_LISTEN_ENDPOINT: IpListenEndpoint = IpListenEndpoint {
    addr: None,
    port: 0,
};

/// `UNSPECIFIED_IPV6` can be used in uninitialized ipv6 address variable.
pub const UNSPECIFIED_IPV6: IpAddress = IpAddress::Ipv6(Ipv6Addr::UNSPECIFIED);

/// `UNSPECIFIED_ENDPOINT_V6` can be used in uninitialized ipv6 endpoint address variable.
pub const UNSPECIFIED_ENDPOINT_V6: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IPV6, 0);

/// `LOCAL_IPV4` is local loop address, which means that info sent to this addr will return local
/// net card.
pub const LOCAL_IPV4: IpAddress = IpAddress::v4(127, 0, 0, 1);

/// `LOCAL_ENDPOINT_V4` is local loop endpoint address. Its addr is Some([`LOCAL_IPV4`]).
pub const LOCAL_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(LOCAL_IPV4, 0);
