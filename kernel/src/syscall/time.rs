use arch::riscv64::time::get_time_us;
use systype::SyscallResult;
use time::{TMS, TimeVal};

use crate::{processor::current_task, vm::user_ptr::UserWritePtr};

pub fn sys_gettimeofday(tv: usize, _tz: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock();
    let mut tv_ptr = UserWritePtr::<TimeVal>::new(tv, &mut addrspace);
    if !tv_ptr.is_null() {
        unsafe {
            tv_ptr.write(TimeVal::from_usec(get_time_us()))?;
        }
    }
    Ok(0)
}

pub fn sys_times(tms: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock();
    let mut tms_ptr = UserWritePtr::<TMS>::new(tms, &mut addrspace);
    if !tms_ptr.is_null() {
        unsafe {
            tms_ptr.write(TMS::from_task_time_stat(task.timer_mut()))?;
        }
    }
    Ok(0)
}
