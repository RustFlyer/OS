use osfuture::take_waker;
use smoltcp::socket::tcp::{self};

use crate::{NetPollState, SOCKET_SET, addr::UNSPECIFIED_ENDPOINT_V4, tcp::LISTEN_TABLE};

use super::{STATE_CLOSED, STATE_CONNECTED, STATE_CONNECTING, STATE_LISTENING, core::TcpSocket};
impl TcpSocket {
    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> NetPollState {
        match self.get_state() {
            STATE_CONNECTING => self.poll_connect().await,
            STATE_CONNECTED => self.poll_stream().await,
            STATE_LISTENING => self.poll_listener(),
            STATE_CLOSED => self.poll_closed(),
            _ => NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            },
        }
    }

    /// Poll the status of a TCP connection to determine if it has been
    /// established (successful connection) or failed (closed connection)
    ///
    /// Returning `true` indicates that the socket has entered a stable
    /// state(connected or failed) and can proceed to the next step
    pub(crate) async fn poll_connect(&self) -> NetPollState {
        log::debug!("[poll_connect] {:?}", unsafe { self.handle.get().read() });
        // SAFETY: `self.handle` should be initialized above.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = take_waker().await;
        let writable = SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            match socket.state() {
                tcp::State::SynSent => {
                    // The connection request has been sent but no response
                    socket.register_recv_waker(&waker);
                    false
                }
                // has been received yet
                tcp::State::Established => {
                    self.set_state(STATE_CONNECTED); // connected
                    log::info!(
                        "[TcpSocket::poll_connect] handle {}: connected to {}",
                        handle,
                        socket.remote_endpoint().unwrap(),
                    );
                    true
                }
                _ => {
                    unsafe {
                        self.local_addr.get().write(UNSPECIFIED_ENDPOINT_V4);
                        self.peer_addr.get().write(UNSPECIFIED_ENDPOINT_V4);
                    }
                    self.set_state(STATE_CLOSED); // connection failed
                    true
                }
            }
        });

        // if writable {
        //     log::debug!("[poll_connect] incoming_tcp_packet");
        //     let local = unsafe { *self.local_addr.get() };
        //     let peer = unsafe { *self.peer_addr.get() };
        //     let mut sockets = SOCKET_SET.0.lock();
        //     LISTEN_TABLE.incoming_tcp_packet(local, peer, &mut sockets);
        // }

        NetPollState {
            readable: false,
            writable,
            hangup: false,
        }
    }

    pub(crate) async fn poll_stream(&self) -> NetPollState {
        log::debug!("[poll_stream] {:?}", unsafe { self.handle.get().read() });
        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = take_waker().await;
        SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            // readable 本质上是是否应该继续阻塞，因此为 true 时的条件可以理解为：
            // 1. 套接字已经关闭接收：在这种情况下，即使没有新数据到达，读取操作也不会阻塞，
            //    因为读取会立即返回
            // 2. 套接字中有数据可读：这是最常见的可读情况，表示可以从套接字中读取到数据
            let readable = !socket.may_recv() || socket.can_recv();
            let writable = !socket.may_send() || socket.can_send();
            if !readable {
                socket.register_recv_waker(&waker);
            }
            if !writable {
                socket.register_send_waker(&waker);
            }
            NetPollState {
                readable,
                writable,
                hangup: false,
            }
        })
    }

    pub(crate) fn poll_listener(&self) -> NetPollState {
        log::debug!("[poll_listener] {:?}", unsafe { self.handle.get().read() });
        // SAFETY: `self.local_addr` should be initialized in a listening socket.

        let local_addr = unsafe { self.local_addr.get().read() };
        let readable = LISTEN_TABLE.can_accept(local_addr.port);

        // let listen_handles = self.listen_handles.lock();
        // let readable = listen_handles.iter().any(|&handle| {
        //     SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |sock| {
        //         sock.state() == tcp::State::Established
        //     })
        // });

        NetPollState {
            readable,
            writable: false,
            hangup: false,
        }
    }

    pub(crate) fn poll_closed(&self) -> NetPollState {
        log::debug!("[poll_closed]");
        use tcp::State::*;
        let handle = unsafe { self.handle.get().read() };
        if let Some(handle) = handle {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                log::warn!(
                    "[TcpSocket::poll_closed] handle {handle} state {}",
                    socket.state()
                );
                let hangup = matches!(socket.state(), CloseWait | FinWait2 | TimeWait);
                NetPollState {
                    readable: true,
                    writable: false,
                    hangup,
                }
            })
        } else {
            NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            }
        }
    }
}
