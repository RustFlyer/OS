use crate::TaskTimeStat;

/// TMS is a struct used for organizing time data. It's just
/// used in syscall `sys_times` to pass message from kernel
/// to user space.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TMS {
    tms_utime: usize,
    tms_stime: usize,
    tms_cutime: usize,
    tms_cstime: usize,
}

impl TMS {
    pub fn new(utime: usize, stime: usize, cutime: usize, cstime: usize) -> Self {
        Self {
            tms_utime: utime,
            tms_stime: stime,
            tms_cutime: cutime,
            tms_cstime: cstime,
        }
    }

    pub fn from_task_time_stat(tts: &TaskTimeStat) -> Self {
        let (utime, stime) = tts.user_and_system_time();
        let (cutime, cstime) = tts.child_user_system_time();
        Self {
            tms_utime: utime.as_micros() as usize,
            tms_stime: stime.as_micros() as usize,
            tms_cutime: cutime.as_micros() as usize,
            tms_cstime: cstime.as_micros() as usize,
        }
    }
}
