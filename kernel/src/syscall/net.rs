use alloc::sync::Arc;
use config::vfs::OpenFlags;
use systype::{SysError, SyscallResult};

use crate::{
    net::{
        SocketType,
        addr::{SaFamily, read_sockaddr, write_sockaddr},
        socket::Socket,
    },
    processor::current_task,
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
