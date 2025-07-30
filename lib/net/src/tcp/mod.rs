use crate_interface::call_interface;
use listentable::ListenTable;
use smoltcp::{iface::SocketSet, socket::icmp::Endpoint, wire::IpEndpoint};
use spin::lazy::Lazy;

pub mod core;
pub mod listenentry;
pub mod listentable;
pub mod recvfuture;
pub mod tcppoll;
pub mod tcpsr;
pub mod tcpstate;

// 下面是来自系统调用的how flag
pub const SHUT_RD: u8 = 0;
pub const SHUT_WR: u8 = 1;
pub const SHUT_RDWR: u8 = 2;

/// 表示读方向已关闭（相当于SHUT_RD）
pub const RCV_SHUTDOWN: u8 = 1;
/// 表示写方向已关闭（相当于SHUT_WR）
pub const SEND_SHUTDOWN: u8 = 2;
/// 表示读和写方向都已关闭（相当于SHUT_RDWR）
pub const SHUTDOWN_MASK: u8 = 3;

// State transitions:
// CLOSED -(connect)-> BUSY -> CONNECTING -> CONNECTED -(shutdown)-> BUSY ->
// CLOSED       |
//       |-(listen)-> BUSY -> LISTENING -(shutdown)-> BUSY -> CLOSED
//       |
//        -(bind)-> BUSY -> CLOSED
pub(crate) const STATE_CLOSED: u8 = 0;
pub(crate) const STATE_BUSY: u8 = 1;
pub(crate) const STATE_CONNECTING: u8 = 2;
pub(crate) const STATE_CONNECTED: u8 = 3;
pub(crate) const STATE_LISTENING: u8 = 4;

lazy_static::lazy_static! {
    pub static ref LISTEN_TABLE:  ListenTable = {
        // driver::println!("when LISTEN_TABLE init");
        ListenTable::new()
    };
}

#[crate_interface::def_interface]
pub trait HasSignalIf: Send + Sync {
    fn has_signal() -> bool;
}

pub(crate) fn has_signal() -> bool {
    call_interface!(HasSignalIf::has_signal())
}

/// use in tcp handshake
///
/// wake listening socket and add remote port as entry in ListenTable
pub fn snoop_tcp_packet(
    buf: &[u8],
    is_ethernet: bool,
) -> Result<Option<(IpEndpoint, IpEndpoint)>, smoltcp::wire::Error> {
    use smoltcp::wire::{EthernetFrame, IpProtocol, Ipv4Packet, TcpPacket};

    // let ether_frame = EthernetFrame::new_checked(buf)?;
    // let ipv4_packet = Ipv4Packet::new_checked(ether_frame.payload())?;
    let ipv4_packet = if is_ethernet {
        let ether_frame = EthernetFrame::new_checked(buf)?;
        Ipv4Packet::new_checked(ether_frame.payload())?
    } else {
        Ipv4Packet::new_checked(buf)?
    };
    if ipv4_packet.next_header() == IpProtocol::Tcp {
        let tcp_packet = TcpPacket::new_checked(ipv4_packet.payload())?;
        let src_addr = (ipv4_packet.src_addr(), tcp_packet.src_port()).into();
        let dst_addr = (ipv4_packet.dst_addr(), tcp_packet.dst_port()).into();
        LISTEN_TABLE.syn_wake(dst_addr, tcp_packet.ack());

        let is_first = tcp_packet.syn() && !tcp_packet.ack();
        if is_first {
            // create a socket for the first incoming TCP packet, as the later accept()
            // returns.
            log::info!("[snoop_tcp_packet] receive TCP");
            LISTEN_TABLE.incoming_tcp_packet(src_addr, dst_addr);
            return Ok(Some((src_addr, dst_addr)));
        }
    }
    Ok(None)
}
