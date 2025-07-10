#![no_std]
#![no_main]
#![feature(ip)]
#![allow(unused)]

use alloc::{boxed::Box, sync::Arc, vec};
use driver::net::NetDevice;
use interface::InterfaceWrapper;
use smoltcp::wire::{EthernetAddress, IpCidr};
use socketset::SocketSetWrapper;
use spin::{lazy::Lazy, once::Once};
use timer::{IEvent, Timer, TimerState};

extern crate alloc;

pub mod addr;
pub mod bench;
pub mod device;
pub mod interface;
pub mod portmap;
pub mod rttoken;
pub mod socketset;
pub mod tcp;
pub mod udp;
pub mod unix;

/// Some meaningless parameters. They will be parsed as bytes
/// and mix with `RANDOM_SEED` to create ips and gateway address.
const IP: &str = "192.168.0.100";
const GATEWAY: &str = "192.168.0.1";
const IP_PREFIX: u8 = 24;

/// `SOCKET_SET` is a global socket manager, used to manage multi-type sockets,
/// such as tcp, udp and unix.
pub(crate) static SOCKET_SET: Lazy<SocketSetWrapper> = Lazy::new(SocketSetWrapper::new);

/// `ETH0` is a wrapper of network card and protocol stack. It can poll receiving
/// and sending event. Also, some important attributes(such as MAC address, gateway
/// address and network card name) about network card is stored in `ETH0`.
pub(crate) static ETH0: Once<InterfaceWrapper> = Once::new();

/// This funtion is used to initialize `ETH0`, setting correct device, ips and gateway.
pub fn init_network(net_dev: Box<dyn NetDevice>, is_loopback: bool) {
    let ether_addr = EthernetAddress(net_dev.mac_address().0);
    let eth0 = InterfaceWrapper::new("eth0", net_dev, ether_addr);

    let gateway = GATEWAY.parse().unwrap();
    let ip_addrs = if is_loopback {
        let ip = "127.0.0.1".parse().unwrap();
        vec![IpCidr::new(ip, 8)]
    } else {
        let ip = IP.parse().expect("invalid IP address");
        vec![
            IpCidr::new(IP.parse().unwrap(), 8),
            IpCidr::new(ip, IP_PREFIX),
        ]
    };

    eth0.setup_ip_addr(ip_addrs);
    eth0.setup_gateway(gateway);

    ETH0.call_once(|| eth0);
}

/// net poll results, used for referring udp/tcp poll state.
#[derive(Debug, Default, Clone, Copy)]
pub struct NetPollState {
    /// Object can be read now.
    pub readable: bool,
    /// Object can be writen now.
    pub writable: bool,
    /// Object is hang up now.
    pub hangup: bool,
}

/// Poll the network stack.
///
/// It may receive packets from the NIC and process them, and transmit queued
/// packets to the NIC.
pub fn poll_interfaces() -> smoltcp::time::Instant {
    SOCKET_SET.poll_interfaces()
}

pub fn net_bench() {
    ETH0.get().unwrap().bench_test();
}

#[derive(Debug)]
struct PollTimer;

impl IEvent for PollTimer {
    fn callback(self: Arc<Self>) -> TimerState {
        poll_interfaces();
        TimerState::Cancelled
    }
}

pub fn net_device_exist() -> bool {
    ETH0.get().is_some()
}
