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

/// 进程时间统计结构体
///
/// 包含用户CPU时间、系统CPU时间、终止子进程的用户CPU时间和系统CPU时间
pub struct TMS {
    tms_utime: usize,  // 调用者用户CPU时间
    tms_stime: usize,  // 调用者系统CPU时间
    tms_cutime: usize, // 终止子进程的用户CPU时间
    tms_cstime: usize, // 终止子进程的系统CPU时间
}

impl TimeSpec {
    /// 纳秒数
    pub const NANO_PER_SEC: usize = 1_000_000_000;

    /// 创建一个新的TimeSpec
    ///
    /// 返回一个新的TimeSpec，包含秒数和纳秒数
    pub fn new(sec: usize, nsec: usize) -> Self {
        Self {
            tv_sec: sec,
            tv_nsec: nsec,
        }
    }

    /// 从毫秒数创建一个新的TimeSpec
    ///
    /// 返回一个新的TimeSpec，包含毫秒数转换的秒数和纳秒数
    pub fn from_ms(ms: usize) -> Self {
        Self {
            tv_sec: ms / 1000,
            tv_nsec: (ms % 1000) * 1_000_000,
        }
    }

    /// 转换为毫秒数
    ///
    /// 返回TimeSpec转换的毫秒数
    pub fn into_ms(&self) -> usize {
        self.tv_sec * 1_000 + self.tv_nsec / 1_000_000
    }
}

impl TimeValue for TimeSpec {
    /// 检查TimeSpec是否有效
    ///
    /// 返回TimeSpec是否有效
    fn is_valid(&self) -> bool {
        self.tv_nsec < Self::NANO_PER_SEC
    }

    /// 检查TimeSpec是否为零
    ///
    /// 返回TimeSpec是否为零
    fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_nsec == 0
    }
}

impl From<Duration> for TimeSpec {
    /// 从Duration创建一个新的TimeSpec
    ///
    /// 返回一个新的TimeSpec，包含Duration的秒数和纳秒数
    fn from(duration: Duration) -> Self {
        Self {
            tv_sec: duration.as_secs() as usize,
            tv_nsec: duration.subsec_nanos() as usize,
        }
    }
}

impl From<TimeSpec> for Duration {
    /// 从TimeSpec创建一个新的Duration
    ///
    /// 返回一个新的Duration，包含TimeSpec的秒数和纳秒数
    fn from(time_spec: TimeSpec) -> Self {
        Duration::new(time_spec.tv_sec as u64, time_spec.tv_nsec as u32)
    }
}

impl TimeVal {
    /// 微秒数
    pub const MICRO_PER_SEC: usize = 1_000_000;

    /// 创建一个新的TimeVal
    ///
    /// 返回一个新的TimeVal，包含秒数和微秒数
    pub fn new(sec: usize, usec: usize) -> Self {
        Self {
            tv_sec: sec,
            tv_usec: usec,
        }
    }

    /// 从微秒数创建一个新的TimeVal
    ///
    /// 返回一个新的TimeVal，包含微秒数转换的秒数和微秒数
    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / Self::MICRO_PER_SEC,
            tv_usec: usec % Self::MICRO_PER_SEC,
        }
    }

    /// 转换为微秒数
    ///
    /// 返回TimeVal转换的微秒数
    pub fn into_usec(&self) -> usize {
        self.tv_sec * Self::MICRO_PER_SEC + self.tv_usec
    }

    pub fn get_time_from_us(&mut self, us: usize) {
        self.tv_sec = us / 1_000_000;
        self.tv_usec = us % 1_000_000;
    }
}

impl TimeValue for TimeVal {
    /// 检查TimeVal是否有效
    ///
    /// 返回TimeVal是否有效
    fn is_valid(&self) -> bool {
        self.tv_usec < Self::MICRO_PER_SEC
    }

    /// 检查TimeVal是否为零
    ///
    /// 返回TimeVal是否为零
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

impl From<usize> for TimeVal {
    fn from(value: usize) -> Self {
        Self {
            tv_sec: value / 1_000_000,
            tv_usec: value % 1_000_000,
        }
    }
}

/// ITimerVal结构体
///
/// 包含定时器间隔和值
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ITimerVal {
    pub it_interval: TimeVal,
    pub it_value: TimeVal,
}

/// ITimerVal实现
impl ITimerVal {
    /// 创建一个新的ITimerVal
    ///
    /// 返回一个新的ITimerVal，包含定时器间隔和值
    pub fn new(interval: TimeVal, value: TimeVal) -> Self {
        Self {
            it_interval: interval,
            it_value: value,
        }
    }

    /// 检查ITimerVal是否有效
    ///
    /// 返回ITimerVal是否有效
    pub fn is_valid(&self) -> bool {
        self.it_interval.is_valid() && self.it_value.is_valid()
    }

    /// 检查ITimerVal是否激活
    ///
    /// 返回ITimerVal是否激活
    pub fn is_activated(&self) -> bool {
        !(self.it_interval.is_zero() && self.it_value.is_zero())
    }
}

/// TMS结构体
///
/// 包含用户CPU时间、系统CPU时间、终止子进程的用户CPU时间和系统CPU时间
impl TMS {
    /// 创建一个新的TMS
    ///
    /// 返回一个新的TMS，包含用户CPU时间、系统CPU时间、终止子进程的用户CPU时间和系统CPU时间
    pub fn new(utime: usize, stime: usize, cutime: usize, cstime: usize) -> Self {
        Self {
            tms_utime: utime,
            tms_stime: stime,
            tms_cutime: cutime,
            tms_cstime: cstime,
        }
    }

    /// 从TaskTimeStat创建一个新的TMS
    ///
    /// 返回一个新的TMS，包含用户CPU时间、系统CPU时间、终止子进程的用户CPU时间和系统CPU时间
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
