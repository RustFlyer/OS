use alloc::boxed::Box;
use async_trait::async_trait;
use config::vfs::{OpenFlags, PollEvents};
use net::{poll_interfaces, udp::UdpSocket};
use systype::SysResult;
use vfs::{
    file::{File, FileMeta},
    sys_root_dentry,
};

use super::{SocketType, addr::SaFamily, sock::Sock};

/// Socket is for user, Sock is for kernel.
pub struct Socket {
    /// The type of socket (such as STREAM, DGRAM)
    pub types: SocketType,
    /// The core of a socket, which includes TCP, UDP, or Unix domain sockets
    pub sk: Sock,
    /// File metadata, including metadata information related to sockets
    pub meta: FileMeta,
}

unsafe impl Sync for Socket {}
unsafe impl Send for Socket {}

impl Socket {
    pub fn new(domain: SaFamily, types: SocketType, nonblock: bool) -> Self {
        let sk = match domain {
            SaFamily::AF_UNIX => unimplemented!(),
            SaFamily::AF_INET | SaFamily::AF_INET6 => match types {
                SocketType::STREAM => unimplemented!(),
                SocketType::DGRAM => Sock::Udp(UdpSocket::new()),
                _ => unimplemented!(),
            },
        };
        let flags = if nonblock {
            sk.set_nonblocking();
            OpenFlags::O_RDWR | OpenFlags::O_NONBLOCK
        } else {
            OpenFlags::O_RDWR
        };
        let meta = FileMeta::new(sys_root_dentry());
        *meta.flags.lock() = flags;

        Self { types, sk, meta }
    }

    pub fn from_another(another: &Self, sk: Sock) -> Self {
        let meta = FileMeta::new(sys_root_dentry());
        *meta.flags.lock() = OpenFlags::O_RDWR;
        Self {
            types: another.types,
            sk,
            meta,
        }
    }
}

#[async_trait]
impl File for Socket {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let bytes = self.sk.recvfrom(buf).await.map(|e| e.0)?;
        log::warn!(
            "[Socket::File::read_at] expect to recv: {:?} exact: {bytes}",
            buf.len()
        );
        Ok(bytes)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let bytes = self.sk.sendto(buf, None).await?;
        log::warn!(
            "[Socket::File::write_at] expect to send: {:?} bytes exact: {bytes}",
            buf.len()
        );
        Ok(bytes)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        poll_interfaces();
        let netstate = self.sk.poll().await;
        if events.contains(PollEvents::IN) && netstate.readable {
            res |= PollEvents::IN;
        }
        if events.contains(PollEvents::OUT) && netstate.writable {
            res |= PollEvents::OUT;
        }
        if netstate.hangup {
            log::warn!("[Socket::bask_poll] PollEvents is hangup");
            res |= PollEvents::HUP;
        }
        log::info!("[Socket::base_poll] ret events:{res:?} {netstate:?}");
        res
    }
}
