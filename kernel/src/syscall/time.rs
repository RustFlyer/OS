use arch::riscv64::time::get_time_us;
use systype::SyscallResult;
use time::TimeVal;

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
