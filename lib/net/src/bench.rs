use smoltcp::phy::{Device, RxToken, TxToken};

use crate::{
    InterfaceWrapper,
    device::DeviceWrapper,
    rttoken::{NetRxToken, NetTxToken},
};

const STANDARD_MTU: usize = 1500;

const GB: usize = 1000 * MB;
const MB: usize = 1000 * KB;
const KB: usize = 1000;

/// For Bench Test
///
/// Directly send/receive the original Ethernet frames (original data packets)
/// to determine the maximum throughput capacity of the network card.
impl DeviceWrapper {
    /// `bench_transmit_bandwidth()` obtains a sending Token directly through `self.transmit()`
    /// This is neither `TCP` nor `UDP`, but just a writable `frame` buffer.
    ///
    /// A "seemingly legitimate `Ethernet` frame" data was simply constructed using the fill
    /// and copy_slice methods (the first 14 bytes are the `Ethernet` header, where `EtherType`
    /// is written as `IPv4`, but no protocol is filled in after it, only 1 is filled in).
    ///
    /// Use the `NetTxToken::consume` encapsulation to directly send the buffer to the
    /// physical network card or the corresponding virtual driver of smoltcp.
    ///
    /// Keep sending packets in a loop until the specified number of bytes is reached,
    /// and then count how much `bandwidth` has been sent.
    pub fn bench_transmit_bandwidth(&mut self) {
        // 10 GB
        const MAX_SEND_BYTES: usize = 10 * GB;
        let mut send_bytes: usize = 0;
        let mut past_send_bytes: usize = 0;
        let mut past_time = InterfaceWrapper::current_time();

        // Send bytes
        while send_bytes < MAX_SEND_BYTES {
            if let Some(tx_token) = self.transmit(InterfaceWrapper::current_time()) {
                // log::debug!("try to send bytes");
                NetTxToken::consume(tx_token, STANDARD_MTU, |tx_buf| {
                    tx_buf[0..12].fill(1);
                    // ether type: IPv4
                    tx_buf[12..14].copy_from_slice(&[0x08, 0x00]);
                    tx_buf[14..STANDARD_MTU].fill(1);
                });
                send_bytes += STANDARD_MTU;
            }

            let current_time = InterfaceWrapper::current_time();
            if (current_time - past_time).secs() == 1 {
                let gb = ((send_bytes - past_send_bytes) * 8) / GB;
                let mb = (((send_bytes - past_send_bytes) * 8) % GB) / MB;
                let gib = (send_bytes - past_send_bytes) / GB;
                let mib = ((send_bytes - past_send_bytes) % GB) / MB;
                log::info!(
                    "Transmit: {}.{:03}GBytes, Bandwidth: {}.{:03}Gbits/sec.",
                    gib,
                    mib,
                    gb,
                    mb
                );
                // log::info!("Transmit: total send bytes: {}", send_bytes);
                past_time = current_time;
                past_send_bytes = send_bytes;
            }
        }

        log::info!("Transmit: total send bytes: {}", send_bytes);
    }

    /// `bench_receive_bandwidth()` reads the original packets from the underlying
    /// layer directly through `self.receive()`, without distinguishing the protocols.
    ///
    /// Use `NetRxToken::consume` to obtain the buffer and count the number of
    /// received bytes.
    ///
    /// Continuously receive packets in a loop to measure the total throughput
    /// and bandwidth.
    pub fn bench_receive_bandwidth(&mut self) {
        // 10 GB
        const MAX_RECEIVE_BYTES: usize = 10 * GB;
        let mut receive_bytes: usize = 0;
        let mut past_receive_bytes: usize = 0;
        let mut past_time = InterfaceWrapper::current_time();
        // Receive bytes
        while receive_bytes < MAX_RECEIVE_BYTES {
            if let Some(rx_token) = self.receive(InterfaceWrapper::current_time()) {
                NetRxToken::consume(rx_token.0, |rx_buf| {
                    receive_bytes += rx_buf.len();
                });
            }

            let current_time = InterfaceWrapper::current_time();
            if (current_time - past_time).secs() == 1 {
                let gb = ((receive_bytes - past_receive_bytes) * 8) / GB;
                let mb = (((receive_bytes - past_receive_bytes) * 8) % GB) / MB;
                let gib = (receive_bytes - past_receive_bytes) / GB;
                let mib = ((receive_bytes - past_receive_bytes) % GB) / MB;
                log::info!(
                    "Receive: {}.{:03}GBytes, Bandwidth: {}.{:03}Gbits/sec.",
                    gib,
                    mib,
                    gb,
                    mb
                );
                // log::info!("Receive: total receive bytes: {}", receive_bytes);
                past_time = current_time;
                past_receive_bytes = receive_bytes;
            }
        }

        log::info!("Receive: total receive bytes: {}", receive_bytes);
    }
}
