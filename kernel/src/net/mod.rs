use alloc::string::ToString;
use config::inode::InodeMode;
use smoltcp::wire::{IpAddress, IpListenEndpoint};
use strum::FromRepr;
use systype::error::{SysError, SysResult};

use crate::processor::current_task;

pub mod addr;
pub mod interface;
pub mod msg;
pub mod sock;
pub mod socket;
pub mod sockopt;

#[derive(FromRepr, Debug, PartialEq, Eq, Clone, Copy)]
pub enum SocketType {
    /// TCP
    STREAM = 1,
    /// UDP
    DGRAM = 2,
    RAW = 3,
    RDM = 4,
    SEQPACKET = 5,
    DCCP = 6,
    PACKET = 10,
}

fn check_unix_path(path: &str) -> SysResult<()> {
    let task = current_task();
    if let Some(parent) = path.rfind('/') {
        let dir = &path[..parent];
        let dentry = task
            .walk_at(config::vfs::AtFd::FdCwd, path.to_string())
            .map_err(|_| SysError::ENOTDIR)?;

        if !dentry
            .inode()
            .ok_or(SysError::ENOENT)?
            .get_meta()
            .inner
            .lock()
            .mode
            == InodeMode::DIR
        {
            return Err(SysError::ENOTDIR);
        }
    }
    Ok(())
}

fn is_local_ip(listen_ep: &IpListenEndpoint) -> bool {
    if let Some(addr) = &listen_ep.addr {
        match addr {
            IpAddress::Ipv4(ipv4) => ipv4.is_loopback() || ipv4.is_unspecified(),
            IpAddress::Ipv6(ipv6) => ipv6.is_loopback() || ipv6.is_unspecified(),
        }
    } else {
        true // 0.0.0.0
    }
}
