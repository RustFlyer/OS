use core::time::Duration;

use super::TimeValue;

/// This is a detailed time statistic. Its recorded time accuracy
/// is down to nanoseconds.
#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: usize,
    pub tv_nsec: usize,
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

    pub fn from_us(us: usize) -> Self {
        Self {
            tv_sec: us / 1_000_000,
            tv_nsec: (us % 1_000_000) * 1_000,
        }
    }

    pub fn into_us(&self) -> usize {
        self.tv_sec * 1_000_000 + self.tv_nsec / 1_000
    }

    pub fn from_ns(ns: usize) -> Self {
        Self {
            tv_sec: ns / Self::NANO_PER_SEC,
            tv_nsec: ns % Self::NANO_PER_SEC,
        }
    }

    pub fn into_ns(&self) -> usize {
        self.tv_sec * Self::NANO_PER_SEC + self.tv_nsec
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
    fn from(spec: TimeSpec) -> Self {
        Duration::new(spec.tv_sec as u64, spec.tv_nsec as u32)
    }
}
