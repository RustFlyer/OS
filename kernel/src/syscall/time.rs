use arch::riscv64::time::get_time_us;
use systype::{SysError, SyscallResult};
use time::{TMS, TimeSpec, TimeVal};

use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub async fn sys_gettimeofday(tv: usize, _tz: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock().await;
    let mut tv_ptr = UserWritePtr::<TimeVal>::new(tv, &mut addrspace);
    if !tv_ptr.is_null() {
        unsafe {
            tv_ptr.write(TimeVal::from_usec(get_time_us()))?;
        }
    }
    Ok(0)
}

pub async fn sys_times(tms: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock().await;
    let mut tms_ptr = UserWritePtr::<TMS>::new(tms, &mut addrspace);
    if !tms_ptr.is_null() {
        unsafe {
            tms_ptr.write(TMS::from_task_time_stat(task.timer_mut()))?;
        }
    }
    Ok(0)
}

pub async fn sys_nanosleep(req: usize, rem: usize) -> SyscallResult {
    let task = current_task();
    let req_time = {
        let mut addrspace = task.addr_space_mut().lock().await;
        let mut req_read = UserReadPtr::<TimeSpec>::new(req, &mut addrspace);
        if req_read.is_null() {
            log::info!("[sys_nanosleep] sleep request is null");
            return Ok(0);
        }
        unsafe { req_read.read()? }
    };

    let remain = task.suspend_timeout(req_time.into()).await;
    if remain.is_zero() {
        return Ok(0);
    }

    let mut addrspace = task.addr_space_mut().lock().await;
    let mut rem_write = UserWritePtr::<TimeSpec>::new(rem, &mut addrspace);
    if !rem_write.is_null() {
        unsafe {
            rem_write.write(remain.into())?;
        }
    }
    Err(SysError::EINTR)
}
