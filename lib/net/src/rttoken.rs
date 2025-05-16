use core::cell::RefCell;

use alloc::boxed::Box;
use driver::net::{NetDevice, netbuf::NetBufPtrOps};
use smoltcp::{
    phy::{Medium, RxToken, TxToken},
    socket::tcp,
};

use crate::tcp::LISTEN_TABLE;

/// `NetRxToken` implement `RxToken` trait, which means that
/// this token is the only chance that the kernel can process the packet
/// when kernel receive the packet.
///
/// kernel should consume the packet to get raw data slice.
pub(crate) struct NetRxToken<'a>(
    pub(crate) &'a RefCell<Box<dyn NetDevice>>,
    pub(crate) Box<dyn NetBufPtrOps>,
);

/// `NetTxToken` implement `TxToken` trait, which means that
/// you can have a chance to send the packet and this is the
/// only chance that you can write something into the packet.
///
/// user can write sth to f closure and send it out.
pub(crate) struct NetTxToken<'a>(pub(crate) &'a RefCell<Box<dyn NetDevice>>);

impl RxToken for NetRxToken<'_> {
    /// receive net data and then pass raw data as bytes
    /// to `f` closure.
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let medium = self.0.borrow().capabilities().medium;
        let is_ethernet = medium == Medium::Ethernet;
        crate::tcp::snoop_tcp_packet(self.1.packet(), is_ethernet).ok();

        let mut rx_buf = self.1;
        let result = f(rx_buf.packet_mut());
        self.0.borrow_mut().recycle_rx_buffer(rx_buf).unwrap();
        result
    }
}

impl TxToken for NetTxToken<'_> {
    /// build a transmit buffer with length `len` and
    /// push data into f closure.
    ///
    /// In f closure, data should be converted to a valid
    /// net packet(such as eth data packet).
    ///
    /// when closure return ret, transmit buffer will be sent.
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut dev = self.0.borrow_mut();
        let mut tx_buf = dev.alloc_tx_buffer(len).unwrap();
        log::debug!("[NetTxToken] transmit {:?}", tx_buf);
        let ret = f(tx_buf.packet_mut());
        dev.transmit(tx_buf).unwrap();
        ret
    }
}
