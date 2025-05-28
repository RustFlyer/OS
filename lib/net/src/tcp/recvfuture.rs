use core::{
    pin::Pin,
    task::{Context, Poll},
};

use smoltcp::socket::tcp;

use systype::error::{SysError, SyscallResult};

use super::core::TcpSocket;
use crate::{SOCKET_SET, tcp::RCV_SHUTDOWN};

pub struct TcpRecvFuture<'a> {
    socket: &'a TcpSocket,
    buf: &'a [u8],
}

impl<'a> Future for TcpRecvFuture<'a> {
    type Output = SyscallResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let shutdown = unsafe { *self.socket.shutdown.get() };
        if shutdown & RCV_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::recv] shutdown closed read, recv return 0");
            return Poll::Ready(Ok(0));
        }
        if self.socket.is_connecting() {
            log::warn!("[TcpRecvFuture] may loss waker");
            return Poll::Pending;
        } else if !self.socket.is_connected() && shutdown == 0 {
            log::warn!("socket recv() failed");
            return Poll::Ready(Err(SysError::ENOTCONN));
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.socket.handle.get().read().unwrap() };
        let ret =SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket|{
            log::info!(
                "[TcpSocket::recv] handle{handle} state {} is trying to recv",
                socket.state()
            );
            if !socket.is_active() {
                // not open
                log::warn!("[TcpSocket::recv] socket recv() failed because handle{handle} is not active");
                Poll::Ready(Err(SysError::ECONNREFUSED))
            } else if !socket.may_recv() {
                // connection closed
                Poll::Ready(Ok(0))
            } else if socket.recv_queue() > 0 {
                // data available
                // TODO: use socket.recv(|buf| {...})
                // let mut this = self.get_mut();
                // let len = socket.recv_slice(&mut this.buf).map_err(|_| {
                //     warn!("socket recv() failed, badstate");
                //     SysError::EBADF
                // })?;
                // Poll::Ready(Ok(len))
                Poll::Ready(Ok(0))
            } else {
                // no more data
                log::info!(
                    "[TcpSocket::recv] handle{handle} has no data to recv, register waker and suspend"
                );
                if self.socket.is_nonblocking() {
                    return Poll::Ready(Err(SysError::EAGAIN));
                }
                socket.register_recv_waker(cx.waker());
                Poll::Pending
            }
        });
        SOCKET_SET.poll_interfaces();
        ret
    }
}
