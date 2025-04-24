use core::time::Duration;

use arch::riscv64::time::{get_time_duration, get_time_us};
use osfuture::{Select2Futures, SelectOutput};
use systype::{SysError, SyscallResult};
use time::{TMS, TimeSpec, TimeVal};

use crate::{
    processor::current_task,
    task::{TaskState, sig_members::IntrBySignalFuture},
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

/// `gettimeofday()` get the time as well as a timezone.
///
/// The tv argument is a struct timeval:
/// ```c
///struct timeval {
///    time_t      tv_sec;     /* seconds */
///    suseconds_t tv_usec;    /* microseconds */
///};
/// ```
///
/// The tz argument is a struct timezone:
/// ```c
///struct timezone {
///    int tz_minuteswest;     /* minutes west of Greenwich */
///    int tz_dsttime;         /* type of DST correction */
///};
/// ```
/// The use of the timezone structure is obsolete; the tz argument should normally
/// be specified as NULL.
pub async fn sys_gettimeofday(tv: usize, _tz: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let mut tv_ptr = UserWritePtr::<TimeVal>::new(tv, &addr_space);
    if !tv_ptr.is_null() {
        unsafe {
            tv_ptr.write(TimeVal::from_usec(get_time_us()))?;
        }
    }
    Ok(0)
}

/// `times()` stores the current process times in the struct tms that buf(`tms`) points to.
/// The struct tms is as defined:
/// ```c
///struct tms {
///    clock_t tms_utime;  /* user time */
///    clock_t tms_stime;  /* system time */
///    clock_t tms_cutime; /* user time of children */
///    clock_t tms_cstime; /* system time of children */
///};
/// ```
/// The `tms_utime` field contains the CPU time spent executing instructions of the calling
/// process. The `tms_stime` field contains the CPU time spent executing inside the kernel
/// while performing tasks on behalf of the calling process.
///
/// The `tms_cutime` field contains the sum of the `tms_utime` and `tms_cutime` values for all
/// waited-for terminated children. The `tms_cstime` field contains the sum of the `tms_stime`
/// and `tms_cstime` values for all waited-for terminated children.
///
/// Times for terminated children (and their descendants) are added in at the moment wait(2)
/// or waitpid(2) returns their process ID. In particular, times of grandchildren that the
/// children did not wait for are never seen.
///
/// All times reported are in clock ticks.
pub async fn sys_times(tms: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let mut tms_ptr = UserWritePtr::<TMS>::new(tms, &addr_space);
    if !tms_ptr.is_null() {
        unsafe {
            tms_ptr.write(TMS::from_task_time_stat(task.timer_mut()))?;
        }
    }
    Ok(0)
}

/// `nanosleep()` suspends the execution of the calling thread until either at least the time
/// specified in *req has elapsed, or the delivery of a `signal` that triggers the invocation
/// of a handler in the calling thread or that terminates the process.
///
/// If the call is interrupted by a `signal` handler, `nanosleep()` returns -1, sets errno to
/// EINTR, and writes the remaining time into the structure pointed to by `rem` unless `rem` is NULL.
///
/// The value of *rem can then be used to call `nanosleep()` again and complete the
/// specified pause.
///
/// The structure `timespec` is used to specify intervals of time with nanosecond precision.
/// It is defined as follows:
/// ```c
/// struct timespec {
///     time_t tv_sec;        /* seconds */
///     long   tv_nsec;       /* nanoseconds */
/// };
/// ```
pub async fn sys_nanosleep(req: usize, rem: usize) -> SyscallResult {
    let task = current_task();
    let req_time = {
        let addr_space = task.addr_space();
        let mut req_read = UserReadPtr::<TimeSpec>::new(req, &addr_space);
        if req_read.is_null() {
            log::info!("[sys_nanosleep] sleep request is null");
            return Ok(0);
        }
        unsafe { req_read.read()? }
    };

    task.set_state(TaskState::Interruptable);
    task.set_wake_up_signal(!*task.sig_mask_mut());
    let intr_future = IntrBySignalFuture {
        task: task.clone(),
        mask: *task.sig_mask_mut(),
    };

    let remain = match Select2Futures::new(task.suspend_timeout(req_time.into()), intr_future).await
    {
        SelectOutput::Output1(ret) => Ok(ret),
        SelectOutput::Output2(_) => Err(SysError::EINTR),
    }?;

    if remain.is_zero() {
        return Ok(0);
    }

    let addr_space = task.addr_space();
    let mut rem_write = UserWritePtr::<TimeSpec>::new(rem, &addr_space);
    if !rem_write.is_null() {
        unsafe {
            rem_write.write(remain.into())?;
        }
    }
    Err(SysError::EINTR)
}

// clockid
pub const SUPPORT_CLOCK: usize = 6;
/// A configurable system-level real-time clock for measuring the real (i.e., the wall clock) time
pub const CLOCK_REALTIME: usize = 0;
/// An unsettable system-level clock representing monotonic time since an unspecified past point in time
pub const CLOCK_MONOTONIC: usize = 1;
/// `CLOCK_PROCESS_CPUTIME_ID` is used to measure the CPU time consumed by the calling process
pub const CLOCK_PROCESS_CPUTIME_ID: usize = 2;
/// `CLOCK_THREAD_CPUTIME_ID` is used to measure the CPU time consumed by the calling thread
pub const CLOCK_THREAD_CPUTIME_ID: usize = 3;
/// `CLOCK_REALTIME_COARSE` is Rough version of the system clock.
pub const CLOCK_REALTIME_COARSE: usize = 5;

pub static mut CLOCK_DEVIATION: [Duration; SUPPORT_CLOCK] = [Duration::ZERO; SUPPORT_CLOCK];

/// clock_gettime is used to obtain the current time values of various "clocks" in the Linux/POSIX environment
///
/// # clockid
/// - 0 = `CLOCK_REALTIME`: The system wall clock can be modified at any time by date or ntp (for example,
///   synchronizing the server time will cause it to jump).
/// - 1 = `CLOCK_MONOTONIC`: Monotonically increasing, it accumulates upward after starting from the kernel
///   and does not reverse or jump (most commonly used for measuring intervals/timing).
/// - 2 = `CLOCK_PROCESS_CPUTIME_ID`: The total CPU time consumed by the calling process, excluding sleep.
/// - 3 = `CLOCK_THREAD_CPUTIME_ID`: The CPU time consumed by the calling thread.
/// - 4 = `CLOCK_MONOTONIC_RAW`: The original value of the monotonic clock is not affected by ntp or adjustments.
/// - 5 = `CLOCK_REALTIME_COARSE`: Rough version of the system clock.
pub fn sys_clock_gettime(clockid: usize, tp: usize) -> SyscallResult {
    log::info!("[sys_clock_gettime] clockid: {clockid}, tp address: {tp:#x}");
    let task = current_task();
    let addr_space = task.addr_space();
    let mut ts_ptr = UserWritePtr::<TimeSpec>::new(tp, &addr_space);

    if ts_ptr.is_null() {
        return Ok(0);
    }

    match clockid {
        CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_REALTIME_COARSE => {
            let current = get_time_duration();
            unsafe {
                ts_ptr.write((CLOCK_DEVIATION[clockid] + current).into())?;
            }
        }
        CLOCK_PROCESS_CPUTIME_ID => {
            let cpu_time = task.get_process_cputime();
            unsafe {
                ts_ptr.write(cpu_time.into())?;
            }
        }
        CLOCK_THREAD_CPUTIME_ID => unsafe {
            ts_ptr.write(task.timer_mut().cpu_time().into())?;
        },
        _ => {
            log::error!("[sys_clock_gettime] unsupported clockid{}", clockid);
            return Err(SysError::EINTR);
        }
    }
    Ok(0)
}
