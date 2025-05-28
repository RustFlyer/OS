use mutex::sleep_mutex::take_waker;
use osfuture::yield_now;
use smoltcp::socket::tcp;
use systype::error::{SysError, SysResult};
use timer::sleep_ms;

use crate::{SOCKET_SET, socketset::TCP_TX_BUF_LEN};

use super::{RCV_SHUTDOWN, SEND_SHUTDOWN, core::TcpSocket};

impl TcpSocket {
    /// Receives data from the socket, stores it in the given buffer.
    pub async fn recv(&self, buf: &mut [u8]) -> SysResult<usize> {
        let shutdown = unsafe { *self.shutdown.get() };
        if shutdown & RCV_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::recv] shutdown closed read, recv return 0");
            return Ok(0);
        }

        if self.is_connecting() {
            return Err(SysError::EAGAIN);
        } else if !self.is_connected() && shutdown == 0 {
            log::warn!("socket recv() failed");
            return Err(SysError::ENOTCONN);
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = take_waker().await;
        self.block_on(|| {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                log::info!("[TcpSocket::recv] handle{handle} state {} is trying to recv", socket.state());
                if !socket.is_active() {
                    // not open
                    log::warn!("[TcpSocket::recv] socket recv() failed because handle{handle} is not active");
                    Err(SysError::ECONNREFUSED)
                } else if !socket.may_recv() {
                    log::error!("[TcpSocket::recv] connection closed");
                    // connection closed
                    Ok(0)
                } else if socket.recv_queue() > 0 {
                    // data available
                    // TODO: use socket.recv(|buf| {...})
                    let len = socket.recv_slice(buf).map_err(|_| {
                        log::warn!("socket recv() failed, badstate");
                        SysError::EBADF
                    })?;
                    Ok(len)
                } else {
                    // no more data
                    log::info!("[TcpSocket::recv] handle{handle} has no data to recv, register waker and suspend");
                    socket.register_recv_waker(&waker);
                    Err(SysError::EAGAIN)
                }
            })
        })
        .await
    }

    /// Transmits data in the given buffer.
    pub async fn send(&self, buf: &[u8]) -> SysResult<usize> {
        let shutdown = unsafe { *self.shutdown.get() };
        if shutdown & SEND_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::send] shutdown closed write, send return 0");
            return Ok(0);
        }
        if self.is_connecting() {
            return Err(SysError::EAGAIN);
        } else if !self.is_connected() && shutdown == 0 {
            log::warn!("socket send() failed");
            return Err(SysError::ENOTCONN);
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = take_waker().await;
        let ret = self.block_on(|| {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                if !socket.is_active() || !socket.may_send() {
                    // closed by remote
                    log::warn!("socket send() failed, ECONNRESET");
                    Err(SysError::ECONNRESET)
                } else if socket.can_send() {
                    // connected, and the tx buffer is not full
                    // TODO: use socket.send(|buf| {...})
                    let len = socket.send_slice(buf).map_err(|e| {
                        log::error!("socket recv() failed: bad state, {e:?}");
                        // TODO: Not sure what error should it be
                        SysError::EBADF
                    })?;
                    Ok(len)
                } else {
                    // tx buffer is full
                    log::info!("[TcpSocket::send] handle{handle} send buffer is full, register waker and suspend");
                    socket.register_send_waker(&waker);
                    Err(SysError::EAGAIN)
                }
            })
        })
        .await;
        if let Ok(bytes) = ret {
            if bytes > TCP_TX_BUF_LEN / 2 {
                sleep_ms(3).await;
            } else {
                yield_now().await;
            }
        }
        SOCKET_SET.poll_interfaces();
        ret
    }
}
