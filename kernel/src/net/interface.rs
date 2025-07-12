use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use net::{externf::NetSocketIf, udp::UdpSocket};

use crate::{
    net::{sock::Sock, socket::Socket},
    processor::current_task,
};

pub fn check_socket_is_reuse_by_fd(fd: usize) -> Option<bool> {
    let task = current_task();
    let file = task.with_mut_fdtable(|table| table.get_file(fd)).ok()?;
    if let Some(socket) = file.downcast_arc::<Socket>().ok() {
        if let Sock::Udp(sock) = &socket.sk {
            return Some(sock.reuse_addr.load(Ordering::Relaxed));
        }
    }

    None
}

pub struct NetCheckSocket;
#[crate_interface::impl_interface]
impl NetSocketIf for NetCheckSocket {
    fn check_socket_reuseaddr(fd: usize) -> Option<bool> {
        check_socket_is_reuse_by_fd(fd)
    }
}
