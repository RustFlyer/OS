use alloc::{sync::Arc, vec::Vec};

use config::{mm::PAGE_SIZE, vfs::OpenFlags};
use net::poll_interfaces;
use osfs::pipe::new_pipe;
use systype::error::{SysError, SyscallResult};

use crate::{
    net::{
        SocketType,
        addr::{SaFamily, SockAddr, read_sockaddr, write_sockaddr},
        msg::{IoVec, MmsgHdr},
        sock::Sock,
        socket::Socket,
        sockopt::{SocketLevel, SocketOpt, TcpSocketOpt},
    },
    processor::current_task,
    task::TaskState,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub const NONBLOCK: i32 = 0x800;
pub const CLOEXEC: i32 = 0x80000;

pub fn sys_socket(domain: usize, types: i32, protocal: usize) -> SyscallResult {
    // if domain == 1 {
    //     log::error!("not support unix socket");
    //     return Err(SysError::ENOSYS);
    // }
    let domain = SaFamily::try_from(domain as u16)?;
    log::info!("[sys_socket] new socket {domain:?} {types:#x} protocal:{protocal:#x}");

    let mut types = types;
    let mut flags = OpenFlags::empty();
    let mut nonblock = false;

    if types & NONBLOCK != 0 {
        nonblock = true;
        types &= !NONBLOCK;
        flags |= OpenFlags::O_NONBLOCK;
    }

    if types & CLOEXEC != 0 {
        types &= !CLOEXEC;
        flags |= OpenFlags::O_CLOEXEC;
    }

    let types = SocketType::from_repr(types as usize).ok_or(SysError::EINVAL)?;

    let socket = Socket::new(domain, types, nonblock)?;
    let fd = current_task().with_mut_fdtable(|table| table.alloc(Arc::new(socket), flags))?;
    log::info!("[sys_socket] new socket {types:?} {flags:?} in fd {fd}, nonblock:{nonblock}");
    Ok(fd)
}

pub fn sys_bind(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let local_addr = read_sockaddr(addrspace, addr, addrlen)?;

    log::debug!(
        "[sys_bind] thread: {}, sockfd: {}, addr: {:#x}",
        task.tid(),
        sockfd,
        addr
    );

    // log::debug!("[sys_bind] local_addr: {:?}", local_addr.as_endpoint());
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    socket.sk.bind(sockfd, local_addr)?;
    Ok(0)
}

pub fn sys_getsockname(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let local_addr = socket.sk.local_addr()?;
    write_sockaddr(addrspace, addr, addrlen, local_addr)?;
    Ok(0)
}

/// Allow users to configure sockets
/// But since these configurations are too detailed, they are currently not
/// supported
pub fn sys_setsockopt(
    sockfd: usize,
    level: usize,
    optname: usize,
    optval: usize,
    optlen: usize,
) -> SyscallResult {
    log::info!(
        "[sys_setsockopt] fd: {sockfd} {level:#x} {optname:#x} optval:{optval:#x} optlen:{optlen}",
    );
    let task = current_task();
    let addrspace = task.addr_space();
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let level = SocketLevel::try_from(level)?;
    let optname = SocketOpt::try_from(optname)?;

    // 只支持 SOL_SOCKET 层的 SO_REUSEADDR/SO_REUSEPORT
    if level == SocketLevel::SOL_SOCKET {
        match optname {
            SocketOpt::REUSEADDR => {
                let val = if optlen >= 4 {
                    let mut buf = [0u8; 4];
                    unsafe {
                        let mut ptr = UserReadPtr::<u8>::new(optval, &addrspace);
                        buf = *ptr.read_array(4)?.to_vec().as_array().unwrap();
                    }
                    u32::from_ne_bytes(buf) != 0
                } else {
                    false
                };
                socket.sk.set_reuse_addr(val);
            }
            SocketOpt::REUSEPORT => {
                let val = if optlen >= 4 {
                    let mut buf = [0u8; 4];
                    unsafe {
                        let mut ptr = UserReadPtr::<u8>::new(optval, &addrspace);
                        buf = *ptr.read_array(4)?.to_vec().as_array().unwrap();
                    }
                    u32::from_ne_bytes(buf) != 0
                } else {
                    false
                };
                socket.sk.set_reuse_port(val);
            }
            _ => {
                log::warn!("[setsockopt] unsupported optname: {:?}", optname);
                // return Err(SysError::ENOPROTOOPT);
            }
        }
    }
    Ok(0)
}

pub fn sys_getsockopt(
    _sockfd: usize,
    level: usize,
    optname: usize,
    optval: usize,
    optlen: usize,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    match SocketLevel::try_from(level)? {
        SocketLevel::SOL_SOCKET => {
            const SEND_BUFFER_SIZE: usize = 64 * 1024;
            const RECV_BUFFER_SIZE: usize = 64 * 1024;
            let mut optval = UserWritePtr::<u32>::new(optval, &addrspace);
            let mut optlen = UserWritePtr::<u32>::new(optlen, &addrspace);
            unsafe {
                match SocketOpt::try_from(optname)? {
                    SocketOpt::RCVBUF => {
                        optval.write(RECV_BUFFER_SIZE as u32)?;
                        optlen.write(core::mem::size_of::<u32>() as u32)?;
                    }
                    SocketOpt::SNDBUF => {
                        optval.write(SEND_BUFFER_SIZE as u32)?;
                        optlen.write(core::mem::size_of::<u32>() as u32)?;
                    }
                    SocketOpt::ERROR => {
                        optval.write(0)?;
                        optlen.write(core::mem::size_of::<u32>() as u32)?;
                    }
                    opt => {
                        log::error!("[sys_getsockopt] unsupported SOL_SOCKET opt {opt:?}")
                    }
                };
            }
        }
        SocketLevel::IPPROTO_IP | SocketLevel::IPPROTO_TCP => {
            const MAX_SEGMENT_SIZE: usize = 1460;
            let mut optval = UserWritePtr::<u32>::new(optval, &addrspace);
            let mut optlen = UserWritePtr::<u32>::new(optlen, &addrspace);

            unsafe {
                match TcpSocketOpt::try_from(optname)? {
                    TcpSocketOpt::MAXSEG => {
                        optval.write(MAX_SEGMENT_SIZE as u32)?;
                        optlen.write(size_of::<u32>() as u32)?
                    }
                    TcpSocketOpt::NODELAY => {
                        optval.write(0)?;
                        optlen.write(size_of::<u32>() as u32)?
                    }
                    TcpSocketOpt::INFO => {}
                    TcpSocketOpt::CONGESTION => {
                        log::error!("[sys_getsockopt] TcpSocketOpt::CONGESTION");
                        // optval.write_array("reno".as_bytes() as *const u8)?;
                        optlen.write(0)?
                    }
                };
            }
        }
        SocketLevel::IPPROTO_IPV6 => todo!(),
    }
    Ok(0)
}

/// used to transmit a message to another socket.
///
/// The argument sockfd is the file descriptor of the sending socket.
///
/// If sendto() is used on a connection-mode (SOCK_STREAM, SOCK_SEQPACKET) socket, the arguments
/// dest_addr and addrlen are ignored (and the error EISCONN may be returned when they are not NULL
/// and 0), and the error ENOTCONN is returned when the socket was not actually connected.
///
/// Otherwise, the address of the target is given by dest_addr with addrlen specifying its size.
pub async fn sys_sendto(
    sockfd: usize,
    buf: usize,
    len: usize,
    flags: usize,
    dest_addr: usize,
    addrlen: usize,
) -> SyscallResult {
    // debug_assert!(flags == 0, "unsupported flags");

    let task = current_task();
    log::debug!(
        "[sys_sendto] thread: {}, sockfd: {sockfd:#x}, dest_addr: {dest_addr:#x}",
        task.tid()
    );

    let addrspace = task.addr_space();
    let mut read_ptr = UserReadPtr::<u8>::new(buf, &addrspace);
    let buf = unsafe { read_ptr.try_into_slice(len) }?;
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    task.set_state(TaskState::Interruptible);

    let bytes = match socket.types {
        SocketType::STREAM => {
            if dest_addr != 0 {
                return Err(SysError::EISCONN);
            }
            socket.sk.sendto(buf, None).await?
        }
        SocketType::DGRAM => {
            let sockaddr = if dest_addr != 0 {
                Some(read_sockaddr(addrspace.clone(), dest_addr, addrlen)?)
            } else {
                None
            };
            socket.sk.sendto(buf, sockaddr).await?
        }
        _ => unimplemented!(),
    };

    task.set_state(TaskState::Running);

    poll_interfaces();

    Ok(bytes)
}

pub async fn sys_recvfrom(
    sockfd: usize,
    buf: usize,
    len: usize,
    flags: usize,
    src_addr: usize,
    addrlen: usize,
) -> SyscallResult {
    debug_assert!(flags == 0, "unsupported flags");

    // poll_interfaces();

    let task = current_task();
    let tid = task.tid();
    log::debug!("[sys_recvfrom] tid: {tid} socket fd: {sockfd:#x}, src_addr: {src_addr:#x}");
    log::debug!("[sys_recvfrom] buf: {buf:#x}, len: {len:#x}");
    let addrspace = task.addr_space();
    let mut write_ptr = UserWritePtr::<u8>::new(buf, &addrspace);
    let buf = unsafe { write_ptr.try_into_mut_slice(len) }?;

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let mut temp = vec![0; len];

    task.set_state(TaskState::Interruptible);
    let (bytes, remote_addr) = socket.sk.recvfrom(&mut temp).await?;
    task.set_state(TaskState::Running);

    buf[..bytes].copy_from_slice(&temp[..bytes]);
    write_sockaddr(addrspace.clone(), src_addr, addrlen, remote_addr)?;
    // log::debug!("[sys_recvfrom] recv buf: {:?}", buf);

    Ok(bytes)
}

pub fn sys_listen(sockfd: usize, _backlog: usize) -> SyscallResult {
    let task = current_task();

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    socket.sk.listen()?;
    Ok(0)
}

/// Connect the active socket referenced by the file descriptor `sockfd` to
/// the listening socket specified by `addr` and `addrlen` at the address
pub async fn sys_connect(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    log::info!(
        "[sys_connect] thread: {}, addr: {:#x}, addrlen: {}",
        task.tid(),
        addr,
        addrlen
    );
    let remote_addr = read_sockaddr(addrspace.clone(), addr, addrlen)?;

    // not 0.0.0.0
    if remote_addr.as_unix_path().is_none()
        && !remote_addr.as_listen_endpoint().unwrap().is_specified()
    {
        return Err(SysError::EADDRNOTAVAIL);
    }

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    socket.sk.connect(remote_addr).await?;
    Ok(0)
}

/// The accept() system call accepts an incoming connection on a listening
/// stream socket referred to by the file descriptor `sockfd`. If there are
/// no pending connections at the time of the accept() call, the call
/// will block until a connection request arrives. Both `addr` and
/// `addrlen` are pointers representing peer socket address. if the addrlen
/// pointer is not zero, it will be assigned to the actual size of the
/// peer address.
///
/// On success, the call returns the file descriptor of the newly connected
/// socket.
pub async fn sys_accept(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    log::debug!(
        "[sys_accept]tid: {} sockfd: {} addr: {:#x}",
        task.tid(),
        sockfd,
        addr
    );

    let socket = task.with_mut_fdtable(|table| table.get_file(sockfd))?;
    let socket = socket
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    task.set_state(TaskState::Interruptible);
    task.set_wake_up_signal(!task.get_sig_mask());
    let new_sk = socket.sk.accept().await?;
    task.set_state(TaskState::Running);

    let peer_addr = new_sk.peer_addr()?;
    let peer_addr = SockAddr::from_endpoint(peer_addr);
    write_sockaddr(addrspace, addr, addrlen, peer_addr)?;
    let new_socket = Arc::new(Socket::from_another(&socket, Sock::Tcp(new_sk)));
    let fd = task.with_mut_fdtable(|table| table.alloc(new_socket, OpenFlags::empty()))?;
    Ok(fd)
}

/// Unlike the `close` system call, `shutdown` allows for finer grained
/// control over the closing behavior of connections. `shutdown` can only
/// close the sending and receiving directions of the socket, or both at the
/// same time
pub fn sys_shutdown(sockfd: usize, how: usize) -> SyscallResult {
    let task = current_task();
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    log::info!("[sys_shutdown] sockfd:{sockfd} shutdown {}", match how {
        0 => "READ",
        1 => "WRITE",
        2 => "READ AND WRITE",
        _ => "Invalid argument",
    });

    socket.sk.shutdown(how as u8)?;
    Ok(0)
}

pub fn sys_socketpair(_domain: usize, _types: usize, _protocol: usize, sv: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut sv = UserWritePtr::<[u32; 2]>::new(sv, &addrspace);
    let (pipe_read, pipe_write) = new_pipe(PAGE_SIZE);
    let pipe = task.with_mut_fdtable(|table| {
        let fd_read = table.alloc(pipe_read, OpenFlags::empty())?;
        let fd_write = table.alloc(pipe_write, OpenFlags::empty())?;
        Ok([fd_read as u32, fd_write as u32])
    })?;
    unsafe {
        sv.write(pipe)?;
    }
    Ok(0)
}

pub fn sys_getpeername(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let peer_addr = socket.sk.peer_addr()?;
    log::info!("[sys_getpeername] sockfd: {sockfd}");
    write_sockaddr(addrspace, addr, addrlen, peer_addr)?;
    Ok(0)
}

/// The accept4() system call accepts an incoming connection on a listening
/// stream socket referred to by the file descriptor `sockfd`.
/// The behavior is like sys_accept, but the new socket can be made non-blocking
/// or close-on-exec atomically, based on the `flags` argument.
pub async fn sys_accept4(
    sockfd: usize,
    addr: usize,
    addrlen: usize,
    flags: usize,
) -> SyscallResult {
    pub const SOCK_NONBLOCK: usize = 0x800;
    pub const SOCK_CLOEXEC: usize = 0x80000;
    let task = current_task();
    let addrspace = task.addr_space();
    log::debug!(
        "[sys_accept4]tid: {} sockfd: {} addr: {:#x} flags: {:#x}",
        task.tid(),
        sockfd,
        addr,
        flags
    );

    let supported_flags = SOCK_NONBLOCK | SOCK_CLOEXEC;
    if flags & !supported_flags != 0 {
        return Err(SysError::EINVAL);
    }

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    task.set_state(TaskState::Interruptible);
    task.set_wake_up_signal(!task.get_sig_mask());
    let new_sk = socket.sk.accept().await?;
    task.set_state(TaskState::Running);

    let peer_addr = new_sk.peer_addr()?;
    let peer_addr = SockAddr::from_endpoint(peer_addr);
    write_sockaddr(addrspace, addr, addrlen, peer_addr)?;

    let mut open_flags = OpenFlags::empty();
    if flags & SOCK_NONBLOCK != 0 {
        open_flags |= OpenFlags::O_NONBLOCK;
    }
    if flags & SOCK_CLOEXEC != 0 {
        open_flags |= OpenFlags::O_CLOEXEC;
    }

    let new_socket = Arc::new(Socket::from_another(&socket, Sock::Tcp(new_sk)));
    let fd = task.with_mut_fdtable(|table| table.alloc(new_socket, open_flags))?;
    Ok(fd)
}

/// sendmmsg() system call - send multiple messages on a socket
///
/// The sendmmsg() system call is an extension of sendmsg() that allows
/// the caller to transmit multiple messages on a socket using a single
/// system call, which has performance benefits.
pub async fn sys_sendmmsg(
    sockfd: usize,
    msgvec: usize,
    vlen: usize,
    flags: usize,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    log::debug!(
        "[sys_sendmmsg] tid: {}, sockfd: {}, vlen: {}, flags: {:#x}",
        task.tid(),
        sockfd,
        vlen,
        flags
    );

    if vlen == 0 {
        return Ok(0);
    }

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let mut msgvec_ptr = UserReadPtr::<MmsgHdr>::new(msgvec, &addrspace);
    let msgvec_array = unsafe { msgvec_ptr.read_array(vlen)? };

    let mut sent_count = 0;
    let mut total_bytes = 0;

    task.set_state(TaskState::Interruptible);

    for (i, mmsg) in msgvec_array.iter().enumerate() {
        let msg_hdr = &mmsg.msg_hdr;

        let dest_addr = if msg_hdr.msg_name != 0 && msg_hdr.msg_namelen > 0 {
            Some(read_sockaddr(
                addrspace.clone(),
                msg_hdr.msg_name,
                msg_hdr.msg_namelen as usize,
            )?)
        } else {
            None
        };

        if msg_hdr.msg_iov == 0 || msg_hdr.msg_iovlen == 0 {
            sent_count += 1;
            continue;
        }

        let mut iov_ptr = UserReadPtr::<IoVec>::new(msg_hdr.msg_iov, &addrspace);
        let iov_array = unsafe { iov_ptr.read_array(msg_hdr.msg_iovlen)? };

        let mut buf = Vec::new();
        for iov in iov_array.iter() {
            if iov.iov_len > 0 && iov.iov_base != 0 {
                let mut data_ptr = UserReadPtr::<u8>::new(iov.iov_base, &addrspace);
                let data = unsafe { data_ptr.try_into_slice(iov.iov_len)? };
                buf.extend_from_slice(data);
            }
        }

        let bytes_sent = match socket.types {
            SocketType::STREAM => {
                if dest_addr.is_some() {
                    task.set_state(TaskState::Running);
                    return Err(SysError::EISCONN);
                }
                socket.sk.sendto(&buf, None).await?
            }
            SocketType::DGRAM => socket.sk.sendto(&buf, dest_addr).await?,
            _ => {
                task.set_state(TaskState::Running);
                return Err(SysError::EOPNOTSUPP);
            }
        };

        let mut result_ptr = UserWritePtr::<u32>::new(
            msgvec + i * core::mem::size_of::<MmsgHdr>() + core::mem::offset_of!(MmsgHdr, msg_len),
            &addrspace,
        );

        unsafe {
            result_ptr.write(bytes_sent as u32)?;
        }

        sent_count += 1;
        total_bytes += bytes_sent;

        if bytes_sent < buf.len() {
            break;
        }
    }

    task.set_state(TaskState::Running);
    poll_interfaces();

    log::debug!(
        "[sys_sendmmsg] sent {} messages, total {} bytes",
        sent_count,
        total_bytes
    );

    Ok(sent_count)
}

/// recvmmsg() system call - receive multiple messages from a socket
///
/// The recvmmsg() system call is an extension of recvmsg() that allows
/// the caller to receive multiple messages from a socket using a single
/// system call.
pub async fn sys_recvmmsg(
    sockfd: usize,
    msgvec: usize,
    vlen: usize,
    flags: usize,
    timeout: usize,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    log::debug!(
        "[sys_recvmmsg] tid: {}, sockfd: {}, vlen: {}, flags: {:#x}",
        task.tid(),
        sockfd,
        vlen,
        flags
    );

    if vlen == 0 {
        return Ok(0);
    }

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let mut msgvec_ptr = UserReadPtr::<MmsgHdr>::new(msgvec, &addrspace);
    let msgvec_array = unsafe { msgvec_ptr.read_array(vlen)? };

    let mut recv_count = 0;

    task.set_state(TaskState::Interruptible);

    for (i, mmsg) in msgvec_array.iter().enumerate() {
        let msg_hdr = &mmsg.msg_hdr;

        if msg_hdr.msg_iov == 0 || msg_hdr.msg_iovlen == 0 {
            recv_count += 1;
            continue;
        }

        let mut iov_ptr = UserReadPtr::<IoVec>::new(msg_hdr.msg_iov, &addrspace);
        let iov_array = unsafe { iov_ptr.read_array(msg_hdr.msg_iovlen)? };

        let mut total_len = 0;
        for iov in iov_array.iter() {
            total_len += iov.iov_len;
        }

        if total_len == 0 {
            recv_count += 1;
            continue;
        }

        let mut temp_buf = vec![0u8; total_len];
        let (bytes_received, remote_addr) = socket.sk.recvfrom(&mut temp_buf).await?;

        let mut offset = 0;
        for iov in iov_array.iter() {
            if offset >= bytes_received || iov.iov_len == 0 || iov.iov_base == 0 {
                break;
            }

            let copy_len = core::cmp::min(iov.iov_len, bytes_received - offset);
            let mut data_ptr = UserWritePtr::<u8>::new(iov.iov_base, &addrspace);
            let data_slice = unsafe { data_ptr.try_into_mut_slice(copy_len)? };
            data_slice.copy_from_slice(&temp_buf[offset..offset + copy_len]);
            offset += copy_len;
        }

        if msg_hdr.msg_name != 0 && msg_hdr.msg_namelen > 0 {
            write_sockaddr(
                addrspace.clone(),
                msg_hdr.msg_name,
                msg_hdr.msg_namelen as usize,
                remote_addr,
            )?;
        }

        let mut result_ptr = UserWritePtr::<u32>::new(
            msgvec + i * core::mem::size_of::<MmsgHdr>() + core::mem::offset_of!(MmsgHdr, msg_len),
            &addrspace,
        );
        unsafe {
            result_ptr.write(bytes_received as u32)?;
        }

        recv_count += 1;

        // if block and recv parts of data, continue
        if bytes_received == 0 {
            break;
        }
    }

    task.set_state(TaskState::Running);

    log::debug!("[sys_recvmmsg] received {} messages", recv_count);

    Ok(recv_count)
}
