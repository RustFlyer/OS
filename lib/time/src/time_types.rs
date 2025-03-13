use crate::stat::TaskTimeStat;
use core::time::Duration;

pub trait TimeValue: Copy + Clone + Into<Duration> + From<Duration> {
    fn is_valid(&self) -> bool;
    fn is_zero(&self) -> bool;
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TimeSpec {
    tv_sec: usize,
    tv_nsec: usize,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TimeVal {
    tv_sec: usize,
    tv_usec: usize,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]

/// Process time statistics structure
pub struct TMS {
    tms_utime: usize,  // User CPU time used by caller
    tms_stime: usize,  // System CPU time used by caller
    tms_cutime: usize, // User CPU time of terminated children
    tms_cstime: usize, // System CPU time of terminated children
}

impl TimeSpec {
    pub const NANO_PER_SEC: usize = 1_000_000_000;

    pub fn new(sec: usize, nsec: usize) -> Self {
        Self {
            tv_sec: sec,
            tv_nsec: nsec,
        }
    }

    pub fn from_ms(ms: usize) -> Self {
        Self {
            tv_sec: ms / 1000,
            tv_nsec: (ms % 1000) * 1_000_000,
        }
    }

    pub fn into_ms(&self) -> usize {
        self.tv_sec * 1_000 + self.tv_nsec / 1_000_000
    }
}

impl TimeValue for TimeSpec {
    fn is_valid(&self) -> bool {
        self.tv_nsec < Self::NANO_PER_SEC
    }

    fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_nsec == 0
    }
}

impl From<Duration> for TimeSpec {
    fn from(duration: Duration) -> Self {
        Self {
            tv_sec: duration.as_secs() as usize,
            tv_nsec: duration.subsec_nanos() as usize,
        }
    }
}

impl From<TimeSpec> for Duration {
    fn from(time_spec: TimeSpec) -> Self {
        Duration::new(time_spec.tv_sec as u64, time_spec.tv_nsec as u32)
    }
}

impl TimeVal {
    pub const MICRO_PER_SEC: usize = 1_000_000;

    pub fn new(sec: usize, usec: usize) -> Self {
        Self {
            tv_sec: sec,
            tv_usec: usec,
        }
    }

    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / Self::MICRO_PER_SEC,
            tv_usec: usec % Self::MICRO_PER_SEC,
        }
    }

    pub fn into_usec(&self) -> usize {
        self.tv_sec * Self::MICRO_PER_SEC + self.tv_usec
    }
}

impl TimeValue for TimeVal {
    fn is_valid(&self) -> bool {
        self.tv_usec < Self::MICRO_PER_SEC
    }

    fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_usec == 0
    }
}

impl From<Duration> for TimeVal {
    fn from(duration: Duration) -> Self {
        Self {
            tv_sec: duration.as_secs() as usize,
            tv_usec: duration.subsec_micros() as usize,
        }
    }
}

impl From<TimeVal> for Duration {
    fn from(val: TimeVal) -> Self {
        Duration::new(val.tv_sec as u64, (val.tv_usec * 1000) as u32)
    }
}

impl From<TimeSpec> for TimeVal {
    fn from(spec: TimeSpec) -> Self {
        Self {
            tv_sec: spec.tv_sec,
            tv_usec: spec.tv_nsec / 1000,
        }
    }
}

impl From<TimeVal> for TimeSpec {
    fn from(val: TimeVal) -> Self {
        Self {
            tv_sec: val.tv_sec,
            tv_nsec: val.tv_usec * 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ITimerVal {
    pub it_interval: TimeVal,
    pub it_value: TimeVal,
}

impl ITimerVal {
    pub fn new(interval: TimeVal, value: TimeVal) -> Self {
        Self {
            it_interval: interval,
            it_value: value,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.it_interval.is_valid() && self.it_value.is_valid()
    }

    pub fn is_activated(&self) -> bool {
        !(self.it_interval.is_zero() && self.it_value.is_zero())
    }
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
