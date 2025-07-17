use core::time::Duration;

use alloc::sync::Arc;
use config::vfs::{EpollEvents, OpenFlags};
use osfs::epoll::{
    event::{EpollCtlOp, EpollEvent},
    file::{EpollFile, EpollFuture},
};
use osfuture::{Select2Futures, SelectOutput};
use systype::error::{SysError, SyscallResult};
use timer::{TimedTaskResult, TimeoutFuture};

use crate::{
    processor::current_task,
    task::sig_members::IntrBySignalFuture,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub fn sys_epoll_create1(flags: i32) -> SyscallResult {
    const EPOLL_CLOEXEC: i32 = 0o2000000;

    if flags & !EPOLL_CLOEXEC != 0 {
        return Err(SysError::EINVAL);
    }

    let task = current_task();
    let epoll_file = EpollFile::new();
    let fd_flags = if (flags & EPOLL_CLOEXEC) != 0 {
        OpenFlags::O_CLOEXEC
    } else {
        OpenFlags::empty()
    };

    let fd = task.with_mut_fdtable(|table| table.alloc(Arc::new(epoll_file), fd_flags))?;
    log::debug!("[sys_epoll_create1] created epoll fd: {}", fd);

    Ok(fd)
}

pub fn sys_epoll_ctl(epfd: i32, op: i32, fd: i32, event_ptr: usize) -> SyscallResult {
    let ctl_op = match op {
        1 => EpollCtlOp::Add, // EPOLL_CTL_ADD
        2 => EpollCtlOp::Del, // EPOLL_CTL_DEL
        3 => EpollCtlOp::Mod, // EPOLL_CTL_MOD
        _ => return Err(SysError::EINVAL),
    };

    if fd == epfd {
        return Err(SysError::EINVAL);
    }

    if ctl_op != EpollCtlOp::Del && event_ptr == 0 {
        return Err(SysError::EFAULT);
    }

    let task = current_task();
    let addr_space = task.addr_space();

    let epoll_file = task.with_mut_fdtable(|ft| ft.get_file(epfd as usize))?;
    let epoll_file = epoll_file
        .as_any()
        .downcast_ref::<EpollFile>()
        .ok_or(SysError::EINVAL)?;

    let target_file = task.with_mut_fdtable(|ft| ft.get_file(fd as usize))?;

    let event = if ctl_op == EpollCtlOp::Del {
        EpollEvent {
            events: EpollEvents::empty(),
            data: 0,
        }
    } else {
        let mut user_event_ptr = UserReadPtr::<EpollEvent>::new(event_ptr, &addr_space);
        unsafe { user_event_ptr.read()? }
    };

    epoll_file
        .inner
        .lock()
        .ctl(ctl_op, fd as usize, target_file, event)?;

    log::debug!(
        "[sys_epoll_ctl] epfd={}, op={:?}, fd={}, event={:?}",
        epfd,
        ctl_op,
        fd,
        event
    );

    Ok(0)
}

pub async fn sys_epoll_pwait(
    epfd: i32,
    events_ptr: usize,
    maxevents: i32,
    timeout: i32,
) -> SyscallResult {
    if maxevents <= 0 {
        return Err(SysError::EINVAL);
    }

    log::debug!(
        "[sys_epoll_wait] epfd={}, maxevents={}, timeout={}",
        epfd,
        maxevents,
        timeout
    );

    let task = current_task();
    let addr_space = task.addr_space();

    let epoll_file = task.with_mut_fdtable(|ft| ft.get_file(epfd as usize))?;
    let epoll_file = epoll_file
        .as_any()
        .downcast_ref::<EpollFile>()
        .ok_or(SysError::EINVAL)?;

    let event = epoll_file.inner.lock().clone();
    let epoll_future = EpollFuture::new(event, maxevents as usize);
    let intr_future = IntrBySignalFuture::new(task.clone(), task.get_sig_mask());

    let (inner, ret_vec) = if timeout >= 0 {
        match Select2Futures::new(
            TimeoutFuture::new(Duration::from_millis(timeout as u64), epoll_future),
            intr_future,
        )
        .await
        {
            SelectOutput::Output1(time_output) => match time_output {
                TimedTaskResult::Completed(ret_vec) => ret_vec,
                TimedTaskResult::Timeout => {
                    log::debug!("[sys_epoll_wait]: timeout");
                    return Ok(0);
                }
            },
            SelectOutput::Output2(_) => return Err(SysError::EINTR),
        }
    } else {
        match Select2Futures::new(epoll_future, intr_future).await {
            SelectOutput::Output1(ret_vec) => ret_vec,
            SelectOutput::Output2(_) => return Err(SysError::EINTR),
        }
    };

    *epoll_file.inner.lock() = inner;

    let num_ready = ret_vec.len();

    let mut user_events_ptr = UserWritePtr::<EpollEvent>::new(events_ptr, &addr_space);
    unsafe { user_events_ptr.write_array(&ret_vec)? };

    Ok(num_ready)
}
