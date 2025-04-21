use core::cell::RefCell;

use alloc::boxed::Box;
use driver::net::NetDevice;
use smoltcp::phy::Device;

use crate::rttoken::{NetRxToken, NetTxToken};

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

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        todo!()
    }

    fn receive(
        &mut self,
        timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        todo!()
    }

    fn transmit(&mut self, timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        todo!()
    }
}
