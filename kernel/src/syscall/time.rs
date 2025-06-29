use alloc::sync::Arc;
use core::time::Duration;
use mutex::SpinNoIrqLock;

use arch::time::{get_time_duration, get_time_ms, get_time_us};
use osfuture::{Select2Futures, SelectOutput};
use systype::{
    error::{SysError, SyscallResult},
    time::{ITimerVal, TMS, TimeSpec, TimeVal, TimeValue},
};
use timer::{TIMER_MANAGER, Timer};

use crate::{
    processor::current_task,
    task::{TaskState, sig_members::IntrBySignalFuture, time::RealITimer, timeid::timeid_alloc},
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
            tms_ptr.write(TMS::from(&*task.timer_mut()))?;
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
    log::debug!("[sys_nanosleep] called {}", get_time_ms());

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
        log::debug!("[sys_nanosleep] sleep enough {}", get_time_ms());
        return Ok(0);
    }

    log::debug!("[sys_nanosleep] not sleep enough");
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
    let task = current_task();
    let tid = task.tid();
    log::info!("[sys_clock_gettime] tid: {tid}, clockid: {clockid}, tp address: {tp:#x}");
    let addr_space = task.addr_space();
    let mut ts_ptr = UserWritePtr::<TimeSpec>::new(tp, &addr_space);

    if ts_ptr.is_null() {
        return Ok(0);
    }

    // for _i in 0..1000 {
    //     simdebug::stop();
    // }

    match clockid {
        CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_REALTIME_COARSE => {
            let current = get_time_duration();
            // let current = get_time_duration() * 4;
            unsafe {
                let ts: TimeSpec = (CLOCK_DEVIATION[clockid] + current).into();
                log::info!("[sys_clock_gettime] ts: {ts:?}");
                ts_ptr.write(ts)?;
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

pub async fn sys_clock_nanosleep(
    clockid: usize,
    flags: usize,
    t: usize,
    rem: usize,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut t = UserReadPtr::<TimeSpec>::new(t, &addrspace);
    let mut rem = UserWritePtr::<TimeSpec>::new(rem, &addrspace);

    /// for clock_nanosleep
    pub const TIMER_ABSTIME: usize = 1;
    match clockid {
        CLOCK_REALTIME | CLOCK_MONOTONIC => {
            let ts = unsafe { t.read()? };
            let req: Duration = ts.into();
            log::debug!(
                "[sys_clock_nanosleep] clockid {} ts: {} ms",
                clockid,
                ts.into_ms()
            );
            let remain = if flags == TIMER_ABSTIME {
                let current = get_time_duration();
                if req.le(&current) {
                    return Ok(0);
                }
                let sleep = req - current;
                task.suspend_timeout(sleep).await
            } else {
                task.suspend_timeout(req).await
            };
            if remain.is_zero() {
                Ok(0)
            } else {
                if !rem.is_null() {
                    unsafe {
                        rem.write(remain.into())?;
                    }
                }
                Err(SysError::EINTR)
            }
        }
        _ => {
            log::error!("[sys_clock_nanosleep] unsupported clockid {}", clockid);
            Err(SysError::EINVAL)
        }
    }
}

/// The function setitimer() arms or disarms the timer specified by which, by setting the
/// timer to the value specified by new_value. If old_value is non-NULL, the buffer it
/// points to is used to return the previous value of the timer (i.e., the same information
/// that is returned by getitimer()).
///
/// If either field in new_value.it_value is nonzero, then the timer is armed to initially
/// expire at the specified time. If both fields in new_value.it_value are zero, then
/// the timer is disarmed.
///
/// The new_value.it_interval field specifies the new interval for the timer; if both
/// of its subfields are zero, the timer is single-shot.
/// ```c
/// struct itimerval {
///     struct timeval it_interval; /* Interval for periodic timer */
///     struct timeval it_value;    /* Time until next expiration */
/// };
///
/// struct timeval {
///     time_t      tv_sec;         /* seconds */
///     suseconds_t tv_usec;        /* microseconds */
/// };
/// ```
pub fn sys_setitimer(which: usize, new_itimeval: usize, old_itimeval: usize) -> SyscallResult {
    log::info!(
        "[sys_setitimer] which: {}, new_itimeval at {:#x}, old_itimeval at {:#x}",
        which,
        new_itimeval,
        old_itimeval
    );

    let task = current_task();
    let addrspace = task.addr_space();
    let mut old_itimeval_ptr = UserWritePtr::<ITimerVal>::new(old_itimeval, &addrspace);
    let mut new_itimeval_ptr = UserReadPtr::<ITimerVal>::new(new_itimeval, &addrspace);

    let new_itimeval = unsafe { new_itimeval_ptr.read() }?;

    log::info!(
        "[sys_setitimer] new_itimeval: it_interval = {:?}, it_value = {:?}",
        new_itimeval.it_interval,
        new_itimeval.it_value
    );

    if !new_itimeval.is_valid() {
        return Err(SysError::EINVAL);
    }

    let timerid = timeid_alloc();
    match which {
        CLOCK_REALTIME => {
            let (old_itimeval, expire) = task.with_mut_itimers(|itimers| {
                let itimer = &mut itimers[which];
                let old_itimeval = ITimerVal {
                    it_interval: itimer.interval.into(),
                    it_value: itimer
                        .next_expire
                        .saturating_sub(get_time_duration())
                        .into(),
                };

                // Updating the itimer ID will disable the previous timer if it exists.
                // A better solution is to remove the timer from the timer manager.
                itimer.id = timerid.0;
                itimer.interval = new_itimeval.it_interval.into();

                log::info!(
                    "[sys_setitimer] task {}: itimer id is set to {}",
                    task.pid(),
                    itimer.id
                );

                if new_itimeval.it_value.is_zero() {
                    itimer.next_expire = Duration::ZERO;
                    (old_itimeval, Duration::ZERO)
                } else {
                    itimer.next_expire = get_time_duration() + new_itimeval.it_value.into();
                    (old_itimeval, new_itimeval.it_value.into())
                }
            });

            if !expire.is_zero() {
                log::info!("[sys_setitimer] add a new timer with id {}", timerid.0);
                let rtimer = RealITimer {
                    task: Arc::downgrade(&task),
                    id: timerid.0,
                };
                let mut timer = Timer::new(get_time_duration() + expire);
                timer.set_callback(Arc::new(rtimer));
                TIMER_MANAGER.add_timer(timer);
            }

            if !old_itimeval_ptr.is_null() {
                unsafe { old_itimeval_ptr.write(old_itimeval)? };
            }
        }
        _ => {
            log::error!("[sys_setitimer] not implemented");
        }
    }
    Ok(0)
}

/// The function getitimer() places the current value of the timer specified by which in
/// the buffer pointed to by curr_value.
///
/// The it_value substructure is populated with the amount of time remaining until
/// the next expiration of the specified timer. This value changes as the timer
/// counts down, and will be reset to it_interval when the timer expires.
/// If both fields of it_value are zero, then this timer is currently disarmed
/// (inactive).
///
/// The it_interval substructure is populated with the timer interval. If both fields
/// of it_interval are zero, then this is a single-shot timer (i.e., it expires just once).
pub fn sys_getitimer(which: usize, curr_value: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let mut curr_value = UserWritePtr::<ITimerVal>::new(curr_value, &addrspace);

    if curr_value.is_null() {
        return Ok(0);
    }

    let itimerval = task.with_mut_itimers(|itimers| {
        let itimer = &itimers[which];
        ITimerVal {
            it_interval: itimer.interval.into(),
            it_value: itimer
                .next_expire
                .saturating_sub(get_time_duration())
                .into(),
        }
    });

    unsafe {
        curr_value.write(itimerval)?;
    }

    Ok(0)
}

/// finds the resolution (precision) of the specified clock clockid
pub fn sys_clock_getres(_clockid: usize, res: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut resptr = UserWritePtr::<TimeSpec>::new(res, &addrspace);
    unsafe {
        if !resptr.is_null() {
            resptr.write(Duration::from_nanos(1).into())?;
        }
    }
    Ok(0)
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Timex {
    pub modes: i32,
    pub offset: i64,
    pub freq: i64,
    pub maxerror: i64,
    pub esterror: i64,
    pub status: i32,
    pub constant: i64,
    pub precision: i64,
    pub tolerance: i64,
    pub time: TimeVal,
    pub tick: i64,
    pub ppsfreq: i64,
    pub jitter: i64,
    pub shift: i32,
    pub stabil: i64,
    pub jitcnt: i64,
    pub calcnt: i64,
    pub errcnt: i64,
    pub stbcnt: i64,
    pub tai: i32,
    // Linux struct timex has padding for future expansion, you can add:
    // _filler: [i32; 11],
}

impl Timex {
    pub const fn new() -> Self {
        Timex {
            modes: 0,
            offset: 0,
            freq: 0,
            maxerror: 0,
            esterror: 0,
            status: 0,
            constant: 0,
            precision: 0,
            tolerance: 0,
            time: TimeVal::new(0, 0),
            tick: 0,
            ppsfreq: 0,
            jitter: 0,
            shift: 0,
            stabil: 0,
            jitcnt: 0,
            calcnt: 0,
            errcnt: 0,
            stbcnt: 0,
            tai: 0,
        }
    }
}

pub static SYSTEM_TIMEX: SpinNoIrqLock<Timex> = SpinNoIrqLock::new(Timex::new());

pub fn sys_adjtimex(user_timex: usize) -> SyscallResult {
    // modes
    pub const ADJ_OFFSET: i32 = 0x0001;
    pub const ADJ_FREQUENCY: i32 = 0x0002;
    pub const ADJ_MAXERROR: i32 = 0x0004;
    pub const ADJ_ESTERROR: i32 = 0x0008;
    pub const ADJ_STATUS: i32 = 0x0010;
    pub const ADJ_TIMECONST: i32 = 0x0020;
    pub const ADJ_SETOFFSET: i32 = 0x0100;
    pub const ADJ_TAI: i32 = 0x0080;

    // status
    pub const STA_PLL: i32 = 0x0001;
    pub const STA_PPSFREQ: i32 = 0x0002;
    pub const STA_PPSTIME: i32 = 0x0004;
    pub const STA_FLL: i32 = 0x0008;
    pub const STA_INS: i32 = 0x0010;
    pub const STA_DEL: i32 = 0x0020;
    pub const STA_UNSYNC: i32 = 0x0040;
    pub const STA_FREQHOLD: i32 = 0x0080;
    pub const STA_NANO: i32 = 0x2000;

    let task = current_task();
    let addrspace = task.addr_space();
    let mut timex_ptr = UserWritePtr::<Timex>::new(user_timex, &addrspace);

    let mut user_tm = if !timex_ptr.is_null() {
        let mut read_ptr = UserReadPtr::<Timex>::new(user_timex, &addrspace);
        unsafe { read_ptr.read()? }
    } else {
        Timex::default()
    };

    let mut sys_tm = SYSTEM_TIMEX.lock();

    if user_tm.modes & ADJ_OFFSET != 0 {
        sys_tm.offset = user_tm.offset;
    }
    if user_tm.modes & ADJ_FREQUENCY != 0 {
        sys_tm.freq = user_tm.freq;
    }
    if user_tm.modes & ADJ_MAXERROR != 0 {
        sys_tm.maxerror = user_tm.maxerror;
    }
    if user_tm.modes & ADJ_ESTERROR != 0 {
        sys_tm.esterror = user_tm.esterror;
    }
    if user_tm.modes & ADJ_STATUS != 0 {
        sys_tm.status = user_tm.status;
    }
    if user_tm.modes & ADJ_TIMECONST != 0 {
        sys_tm.constant = user_tm.constant;
    }
    if user_tm.modes & ADJ_TAI != 0 {
        sys_tm.tai = user_tm.tai;
    }

    // update time
    let now = get_time_duration();
    sys_tm.time = now.into();

    user_tm = *sys_tm;

    if !timex_ptr.is_null() {
        unsafe {
            timex_ptr.write(user_tm)?;
        }
    }

    // return: 0 sync, 1 unsynx=c, val < 0 means error
    let ret = if sys_tm.status & STA_UNSYNC != 0 {
        1
    } else {
        0
    };
    Ok(ret)
}
