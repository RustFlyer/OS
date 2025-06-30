use core::cell::RefCell;

use alloc::boxed::Box;
use driver::net::NetDevice;
use smoltcp::{
    phy::{self},
    wire::IpEndpoint,
};

use crate::{
    addr::UNSPECIFIED_ENDPOINT_V4,
    rttoken::{NetRxToken, NetTxToken},
};

#[allow(unused)]
pub struct TcpState {
    pub(crate) is_recv_first: bool,
    pub(crate) src_addr: IpEndpoint,
    pub(crate) dst_addr: IpEndpoint,
}

impl TcpState {
    fn new() -> Self {
        Self {
            is_recv_first: false,
            src_addr: UNSPECIFIED_ENDPOINT_V4,
            dst_addr: UNSPECIFIED_ENDPOINT_V4,
        }
    }
}

/// `DeviceWrapper` is created for convenience to wrap dyn
/// trait and modify inner member.
pub(crate) struct DeviceWrapper {
    inner: RefCell<Box<dyn NetDevice>>,
    pub state: TcpState,
}

impl DeviceWrapper {
    pub fn new(inner: Box<dyn NetDevice>) -> Self {
        Self {
            inner: RefCell::new(inner),
            state: TcpState::new(),
        }
    }
    pub fn _clear_state(&mut self) {
        self.state.is_recv_first = false;
    }
}

impl phy::Device for DeviceWrapper {
    #[rustfmt::skip]
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;

    #[rustfmt::skip]
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;

    /// Gets a description of device capabilities
    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        self.inner.borrow().capabilities()
    }

    /// Constructs a token pair consisting of one `receive` token and one `transmit` token.
    ///
    /// The additional `transmit` token makes it possible to generate a `reply` packet based
    /// on the contents of the received packet. For example, this makes it possible to
    /// handle arbitrarily large ICMP echo ("ping") requests, where the all received bytes
    /// need to be sent back, without heap allocation.
    ///
    /// The `timestamp` must be a number of milliseconds, monotonically increasing
    /// since an arbitrary moment in time, such as system startup.
    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            log::warn!("{e:?}");
            return None;
        }

        if !dev.can_transmit() {
            return None;
        }

        let rx_buf = match dev.receive() {
            Ok(buf) => buf,
            Err(err) => {
                // log::error!("err: {err:?}");
                return None;
            }
        };
        let rxtoken = NetRxToken(&self.inner, rx_buf);

        Some((rxtoken, NetTxToken(&self.inner)))
    }

    /// Constructs a transmit token.
    ///
    /// The timestamp must be a number of milliseconds, monotonically increasing
    /// since an arbitrary moment in time, such as system startup.
    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            log::error!("DevError: {:?}", e);
            return None;
        }
        if dev.can_transmit() {
            Some(NetTxToken(&self.inner))
        } else {
            None
        }
    }
}
