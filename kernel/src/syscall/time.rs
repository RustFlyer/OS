use arch::riscv64::time::get_time_us;
use systype::SyscallResult;
use time::TimeVal;

use crate::processor::current_task;

pub fn sys_gettimeofday(tv: *const TimeVal, _tz: usize) -> SyscallResult {
    let task = current_task();
    if !tv.is_null() {
        unsafe {
            let mut timeval = &mut *(tv as *mut TimeVal);
            timeval.get_time_from_us(get_time_us());
        }
    }
    Ok(0)
}
