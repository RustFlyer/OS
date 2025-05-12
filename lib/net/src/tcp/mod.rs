use crate_interface::call_interface;
use listentable::ListenTable;
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

pub static LISTEN_TABLE: Lazy<ListenTable> = Lazy::new(ListenTable::new);

#[crate_interface::def_interface]
pub trait HasSignalIf: Send + Sync {
    fn has_signal() -> bool;
}

pub(crate) fn has_signal() -> bool {
    call_interface!(HasSignalIf::has_signal())
}
