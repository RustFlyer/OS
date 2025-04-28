use core::{ops::DerefMut, time::Duration};

use alloc::{boxed::Box, vec::Vec};
use arch::riscv64::time::{get_time_duration, get_time_us};
use driver::net::NetDevice;
use mutex::SpinNoIrqLock;
use smoltcp::{
    iface::{Config, Interface, SocketSet},
    phy::Medium,
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr},
};

use crate::device::DeviceWrapper;

type SmolInstant = smoltcp::time::Instant;
type SmolDuration = smoltcp::time::Duration;

const RANDOM_SEED: u64 = 83198713;

/// `InterfaceWrapper` connects the `smoltcp` network protocol stack
/// and the network card `driver` adaptation layer.
///
/// `smoltcp::Interface` manages protocol layer(routing, IP, TCP/UDP distribution),
/// but it does not manage the device lifetime, nor does it automatically configure the
/// local ip/gateway.
///
/// Therefore, this struct `InterfaceWrapper` wraps `smoltcp::Interface` to manage device
/// lifetime and local ip/gateway better.
///
/// # Member
/// - `name`: name of network card, logic name, used to debug(such as "eth0", "usb0")
/// - `ether_addr`: the MAC address of network card.
/// - `dev`: wrapper of network card, used to interact with physical device.
/// - `iface`: smoltcp::Interface
pub(crate) struct InterfaceWrapper {
    name: &'static str,
    ether_addr: EthernetAddress,
    dev: SpinNoIrqLock<DeviceWrapper>,
    iface: SpinNoIrqLock<Interface>,
}

impl InterfaceWrapper {
    /// Creates a new `InterfaceWrapper`. In fact, this function is only called by `ETH0`
    /// global variable. It will set network card's name, refer-dev and MAC address.
    pub(crate) fn new(
        name: &'static str,
        dev: Box<dyn NetDevice>,
        ether_addr: EthernetAddress,
    ) -> Self {
        let mut config = match dev.capabilities().medium {
            Medium::Ethernet => Config::new(HardwareAddress::Ethernet(ether_addr)),
            Medium::Ip => Config::new(HardwareAddress::Ip),
        };
        config.random_seed = RANDOM_SEED;

        let mut dev = DeviceWrapper::new(dev);

        {
            dev.bench_receive_bandwidth();
            dev.bench_transmit_bandwidth();
        }

        let iface = SpinNoIrqLock::new(Interface::new(config, &mut dev, Self::current_time()));
        Self {
            name,
            ether_addr,
            dev: SpinNoIrqLock::new(dev),
            iface,
        }
    }

    /// get current time, just wrap microseconds from `get_time_us()` with `Instant`.
    pub(crate) fn current_time() -> SmolInstant {
        SmolInstant::from_micros_const(get_time_us() as i64)
    }

    /// unwrap Instant and return microseconds `Duration` from `instant`.
    fn ins_to_duration(instant: SmolInstant) -> Duration {
        Duration::from_micros(instant.total_micros() as u64)
    }

    /// unwrap `SmolDuration` and return microseconds `Duration` from `SmolDuration`.
    fn dur_to_duration(duration: SmolDuration) -> Duration {
        Duration::from_micros(duration.total_micros() as u64)
    }

    /// gets the name of network card
    pub fn name(&self) -> &str {
        self.name
    }

    /// gets the ethernet address(MAC address) of network card
    pub fn ethernet_address(&self) -> EthernetAddress {
        self.ether_addr
    }

    /// adds a group of ip addresses `ips` into network card.
    pub fn setup_ip_addr(&self, ips: Vec<IpCidr>) {
        let mut iface = self.iface.lock();
        iface.update_ip_addrs(|ip_addrs| ip_addrs.extend(ips));
    }

    /// adds a `gateway` in the network card. When network card trys to
    /// send a packet to non-local address, this packet will be sent to
    /// the router in (best) `gateway` address to forward.
    pub fn setup_gateway(&self, gateway: IpAddress) {
        let mut iface = self.iface.lock();
        match gateway {
            IpAddress::Ipv4(v4) => iface.routes_mut().add_default_ipv4_route(v4).unwrap(),
            IpAddress::Ipv6(_) => unimplemented!(),
        };
    }

    /// the Most important event handle loop in `Interface`.
    ///
    /// this function handles the sending and receiving of network packets and updating the
    /// protocol stack status.
    ///
    /// return what time it should poll next
    pub fn poll(&self, sockets: &SpinNoIrqLock<SocketSet>) -> SmolInstant {
        let mut dev = self.dev.lock();
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        let timestamp = Self::current_time();
        let _result = iface.poll(timestamp, dev.deref_mut(), &mut sockets);
        timestamp
    }

    /// `check_poll()` checks whether the current time matches the next refresh time in the protocol stack.
    /// It will trigger the protocol stack event or register the next round of events if necessary.
    ///
    /// This function is used to support timer-driven automatic poll (such as in the scenario where smoltcp
    /// needs to wait for a while to receive or send packets)
    pub fn check_poll(&self, timestamp: SmolInstant, sockets: &SpinNoIrqLock<SocketSet>) {
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        match iface
            .poll_delay(timestamp, &mut sockets)
            .map(Self::dur_to_duration)
        {
            Some(Duration::ZERO) => {
                iface.poll(
                    Self::current_time(),
                    self.dev.lock().deref_mut(),
                    &mut sockets,
                );
            }
            Some(delay) => {
                let next_poll = delay + Self::ins_to_duration(timestamp);
                let current = get_time_duration();
                if next_poll < current {
                    iface.poll(
                        Self::current_time(),
                        self.dev.lock().deref_mut(),
                        &mut sockets,
                    );
                } else {
                    // Timer is not supported now.
                    todo!();
                    // let timer = Timer::new(next_poll, Box::new(PollTimer {}));
                    // TIMER_MANAGER.add_timer(timer);
                }
            }
            None => {
                todo!();
                // let timer = Timer::new(
                //     get_time_duration() + Duration::from_millis(2),
                //     Box::new(PollTimer {}),
                // );
                // TIMER_MANAGER.add_timer(timer);
            }
        }
    }
}
