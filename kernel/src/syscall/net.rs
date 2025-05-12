use alloc::{sync::Arc, vec::Vec};
use config::vfs::OpenFlags;
use net::poll_interfaces;
use systype::{SysError, SyscallResult};

use crate::{
    net::{
        SocketType,
        addr::{SaFamily, SockAddr, read_sockaddr, write_sockaddr},
        sock::Sock,
        socket::Socket,
        sockopt::{SocketLevel, SocketOpt},
    },
    processor::current_task,
    task::TaskState,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub const NONBLOCK: i32 = 0x800;
pub const CLOEXEC: i32 = 0x80000;

pub fn sys_socket(domain: usize, types: i32, protocal: usize) -> SyscallResult {
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
    let socket = Socket::new(domain, types, nonblock);
    let fd = current_task().with_mut_fdtable(|table| table.alloc(Arc::new(socket), flags))?;
    log::info!("[sys_socket] new socket {types:?} {flags:?} in fd {fd}, nonblock:{nonblock}");
    Ok(fd)
}

pub fn sys_bind(sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
    log::debug!("[sys_bind] sockfd: {}, addr: {:#x}", sockfd, addr);
    let task = current_task();
    let addrspace = task.addr_space();
    let local_addr = read_sockaddr(addrspace, addr, addrlen)?;

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
    Ok(0)
}

pub fn sys_getsockopt(
    sockfd: usize,
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
            todo!()
        }
        SocketLevel::IPPROTO_IPV6 => todo!(),
    }
    Ok(0)
}

pub async fn sys_sendto(
    sockfd: usize,
    buf: usize,
    len: usize,
    flags: usize,
    dest_addr: usize,
    addrlen: usize,
) -> SyscallResult {
    debug_assert!(flags == 0, "unsupported flags");
    log::debug!("[sys_sendto] socket fd: {sockfd:#x}, dest_addr: {dest_addr:#x}");

    let task = current_task();
    let addrspace = task.addr_space();
    let mut read_ptr = UserReadPtr::<u8>::new(buf, &addrspace);
    let buf = unsafe { read_ptr.try_into_slice(len) }?;
    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    task.set_state(TaskState::Interruptable);

    let bytes = match socket.types {
        SocketType::STREAM => {
            if dest_addr != 0 {
                return Err(SysError::EISCONN);
            }
            socket.sk.sendto(&buf, None).await?
        }
        SocketType::DGRAM => {
            let sockaddr = if dest_addr != 0 {
                Some(read_sockaddr(addrspace.clone(), dest_addr, addrlen)?)
            } else {
                None
            };
            socket.sk.sendto(&buf, sockaddr).await?
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
    log::debug!("[sys_recvfrom] socket fd: {sockfd:#x}, src_addr: {src_addr:#x}");

    poll_interfaces();

    let task = current_task();
    let addrspace = task.addr_space();
    let mut write_ptr = UserWritePtr::<u8>::new(buf, &addrspace);
    let buf = unsafe { write_ptr.try_into_mut_slice(len) }?;

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    let mut temp = Vec::with_capacity(len);
    unsafe { temp.set_len(len) };

    task.set_state(TaskState::Interruptable);
    let (bytes, remote_addr) = socket.sk.recvfrom(&mut temp).await?;
    task.set_state(TaskState::Running);

    buf[..bytes].copy_from_slice(&temp[..bytes]);
    write_sockaddr(addrspace.clone(), src_addr, addrlen, remote_addr)?;
    log::debug!("[sys_recvfrom] recv buf: {:?}", buf);

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
    let remote_addr = read_sockaddr(addrspace.clone(), addr, addrlen)?;

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    log::info!("[sys_connect] fd {sockfd} trys to connect");
    socket.sk.connect(remote_addr).await?;
    // TODO:
    // yield_now().await;
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

    let socket: Arc<Socket> = task
        .with_mut_fdtable(|table| table.get_file(sockfd))?
        .downcast_arc::<Socket>()
        .map_err(|_| SysError::ENOTSOCK)?;

    task.set_state(TaskState::Interruptable);
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
