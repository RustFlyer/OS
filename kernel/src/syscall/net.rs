use alloc::sync::Arc;
use config::vfs::OpenFlags;
use systype::{SysError, SyscallResult};

use crate::{
    net::{SocketType, addr::SaFamily, socket::Socket},
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
