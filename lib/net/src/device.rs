use core::cell::RefCell;

use alloc::boxed::Box;
use driver::net::{DevError, NetDevice};
use smoltcp::phy::Device;

use crate::rttoken::{NetRxToken, NetTxToken};

/// `DeviceWrapper` is created for convenience to wrap dyn
/// trait and modify inner member.
pub(crate) struct DeviceWrapper {
    inner: RefCell<Box<dyn NetDevice>>,
}

impl DeviceWrapper {
    pub fn new(inner: Box<dyn NetDevice>) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}

impl Device for DeviceWrapper {
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
        if let Err(_e) = dev.recycle_tx_buffers() {
            return None;
        }

        if !dev.can_transmit() {
            return None;
        }
        let rx_buf = match dev.receive() {
            Ok(buf) => buf,
            Err(err) => {
                if !matches!(err, DevError::Again) {}
                return None;
            }
        };
        Some((NetRxToken(&self.inner, rx_buf), NetTxToken(&self.inner)))
    }

    /// Constructs a transmit token.
    ///
    /// The timestamp must be a number of milliseconds, monotonically increasing
    /// since an arbitrary moment in time, such as system startup.
    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        let mut dev = self.inner.borrow_mut();
        if let Err(_e) = dev.recycle_tx_buffers() {
            return None;
        }
        if dev.can_transmit() {
            Some(NetTxToken(&self.inner))
        } else {
            None
        }
    }
}
