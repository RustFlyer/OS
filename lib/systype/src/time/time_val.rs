use core::time::Duration;

use super::TimeValue;

/// This is a time statistic. Its recorded time accuracy
/// is down to microseconds.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TimeVal {
    pub(crate) tv_sec: usize,
    pub(crate) tv_usec: usize,
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

    pub fn get_time_from_us(&mut self, us: usize) {
        self.tv_sec = us / 1_000_000;
        self.tv_usec = us % 1_000_000;
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
