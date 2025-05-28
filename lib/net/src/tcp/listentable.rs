use alloc::{boxed::Box, vec::Vec};
use core::{ops::DerefMut, sync::atomic::Ordering, task::Waker};

use smoltcp::{
    iface::{SocketHandle, SocketSet},
    socket::tcp::{self, State},
    wire::{IpEndpoint, IpListenEndpoint},
};

use mutex::{ShareMutex, SpinNoIrqLock};
use systype::error::{SysError, SysResult};

use crate::{SOCKET_SET, socketset::LISTEN_QUEUE_SIZE};

use super::listenentry::{ListenTableEntry, PORT_NUM};

/// A table for managing TCP listen ports.
/// Each index corresponds to a specific port number.
///
/// Using an array allows direct access to the corresponding listen entry
/// through the port number, improving lookup efficiency.
/// A Mutex ensures thread safety, as multiple threads may access and modify
/// the state of the listening ports in a multithreaded environment.
pub struct ListenTable {
    /// An array of Mutexes, each protecting an optional ListenTableEntry for a
    /// specific port.
    tcp: Box<[SpinNoIrqLock<Option<Box<ListenTableEntry>>>]>,
    /// An array of ports, used to store ports in incoming_tcp_packet for future
    /// check after poll.
    waiting_ports: SpinNoIrqLock<Vec<usize>>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(SpinNoIrqLock::new(None));
            }
            buf.assume_init()
        };
        let waiting_ports = SpinNoIrqLock::new(Vec::new());
        Self { tcp, waiting_ports }
    }

    pub fn can_listen(&self, port: u16) -> bool {
        self.tcp[port as usize].lock().is_none()
    }

    pub fn listen(
        &self,
        listen_endpoint: IpListenEndpoint,
        waker: &Waker,
        handles: ShareMutex<Vec<SocketHandle>>,
    ) -> SysResult<()> {
        let port = listen_endpoint.port;
        log::debug!("[listen] port: {}", port);
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock();
        if entry.is_none() {
            *entry = Some(Box::new(ListenTableEntry::new(
                listen_endpoint,
                waker,
                handles,
            )));
        } else {
            log::warn!("socket listen() failed");
            return Err(SysError::EADDRINUSE);
        }

        log::debug!("[listen] {:?}", *entry);

        Ok(())
    }

    pub fn unlisten(&self, port: u16) {
        log::info!("TCP socket unlisten on {}", port);
        if let Some(entry) = self.tcp[port as usize].lock().take() {
            entry.wake()
        }
    }

    pub fn can_accept(&self, port: u16) -> bool {
        if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
            log::debug!("[can_accept] entry.syn_queue: {:?}", entry.syn_queue);
            entry.syn_queue.iter().any(|&handle| is_connected(handle))
            // true
        } else {
            // 因为在listen函数调用时已经将port设为监听状态了，这里应该不会查不到？？
            log::error!("socket accept() failed: not listen. I think this wouldn't happen !!!");
            false
            // Err(SysError::EINVAL)
        }
    }

    /// checks SYN queue in port and find handles which built connection successfully, take them
    /// from the queue and return to caller.
    pub fn accept(&self, port: u16) -> SysResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        log::debug!("[accept] port: {}", port);

        if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
            log::debug!("[accept] entry: {:?}", *entry);
            let syn_queue = &mut entry.syn_queue;
            syn_queue.iter().for_each(|&tuple| {
                log::debug!("[accept] {}, isconnect?{}", tuple, is_connected(tuple))
            });

            let (idx, addr_tuple) = syn_queue
                .iter()
                .enumerate()
                .find_map(|(idx, &handle)| {
                    is_connected(handle).then(|| (idx, get_addr_tuple(handle)))
                })
                .ok_or(SysError::EAGAIN)?; // wait for connection

            // 记录慢速SYN队列遍历的警告信息是为了监控和诊断性能问题
            // 理想情况: 如果网络连接正常，
            // SYN队列中的连接请求应尽快完成三次握手并从队列前端被取出。因此，
            // 最常见的情况是已连接的句柄在队列的前端，即索引为0。
            // 异常情况: 如果队列中第一个元素（索引为0）的连接请求没有完成，
            // 而后续的某个连接请求已经完成，这可能表明存在性能问题或异常情况,如网络延迟、
            // 资源争用
            if idx > 0 {
                log::warn!(
                    "slow SYN queue enumeration: index = {}, len = {}!",
                    idx,
                    syn_queue.len()
                );
            }
            log::debug!("[accept] success return resource");
            let handle = syn_queue.swap_remove_front(idx).unwrap();
            Ok((handle, addr_tuple))
        } else {
            log::warn!("socket accept() failed: not listen");
            Err(SysError::EINVAL)
        }
    }

    pub fn incoming_tcp_packet(&self, src: IpEndpoint, dst: IpEndpoint) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().deref_mut() {
            if !entry.can_accept(dst.addr) {
                // not listening on this address
                log::warn!(
                    "[ListenTable::incoming_tcp_packet] not listening on address {}",
                    dst.addr
                );
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                log::warn!("SYN queue overflow!");
                return;
            }
            entry.waker.wake_by_ref();
            log::info!(
                "[ListenTable::incoming_tcp_packet] wake the socket who listens port {}",
                dst.port
            );

            self.waiting_ports.lock().push(dst.port as usize);

            // let mut socket = SocketSetWrapper::new_tcp_socket();
            // if socket.listen(entry.listen_endpoint).is_ok() {
            //     let handle = sockets.add(socket);
            //     log::info!(
            //         "TCP socket {}: prepare for connection {} -> {}",
            //         handle,
            //         src,
            //         entry.listen_endpoint
            //     );
            //     entry.syn_queue.push_back(handle);
            // }
        }
    }

    pub fn check_after_poll(&self, sockets: &mut SocketSet<'_>) {
        // log::debug!("[check_after_poll] poll");
        let mut list = self.waiting_ports.lock();
        // log::debug!("[check_after_poll] get list lock");
        while !list.is_empty() {
            let port = list.pop().unwrap();
            if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
                log::debug!("[check_after_poll] port: {}", port);
                let mut listen_handles = entry.handles.lock();
                let mut ret = None;
                for (i, &handle) in listen_handles.iter().enumerate() {
                    log::error!("[check_after_poll] port: {}, handle: {}", port, handle);
                    let sock: &mut tcp::Socket<'_> = sockets.get_mut(handle);
                    // SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |sock| {
                    log::debug!("[check_after_poll] sock.state()? {}", sock.state());
                    if sock.state() == State::SynReceived {
                        log::debug!("[check_after_poll] success get handle!");
                        entry.syn_queue.push_back(handle);

                        entry.syn_recv_sleep.store(true, Ordering::Relaxed);
                        let local_addr = sock.local_endpoint().unwrap();
                        ret = Some((handle, local_addr, i));
                    }
                    // });

                    if ret.is_some() {
                        break;
                    }
                }

                if let Some((handle, local_addr, index)) = ret {
                    listen_handles.remove(index);
                    // let new_handle = SOCKET_SET.add(SocketSetWrapper::new_tcp_socket());
                    // SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(new_handle, |sock| {
                    //     sock.listen(local_addr).unwrap();
                    // });
                    // listen_handles.push(new_handle);
                }
            }
        }
    }

    pub fn syn_wake(&self, dst: IpEndpoint, ack: bool) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().deref_mut() {
            // let is_syn = entry.syn_recv_sleep.load(Ordering::Relaxed);
            // if is_syn {
            //     entry
            //         .syn_queue
            //         .iter()
            //         .any(|&handle| is_connected(handle))
            //         .then(|| {
            //             log::debug!("[syn_wake] is connected");
            //             entry.syn_recv_sleep.store(false, Ordering::Relaxed);
            //             log::debug!("[syn_wake] wake process");
            //             entry.waker.wake_by_ref();
            //         });
            // }
            ack.then(|| {
                entry.waker.wake_by_ref();
            });
        }
    }
}

fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
        log::debug!("socket.state(): {}", socket.state());
        !matches!(socket.state(), tcp::State::Listen | tcp::State::SynReceived)
    })
}

fn get_addr_tuple(handle: SocketHandle) -> (IpEndpoint, IpEndpoint) {
    SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
        (
            socket.local_endpoint().unwrap(),
            socket.remote_endpoint().unwrap(),
        )
    })
}
