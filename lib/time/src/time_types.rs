use crate::{timespec::TimeSpec, timeval::TimeVal};
use core::time::Duration;

pub trait TimeValue: Copy + Clone + Into<Duration> + From<Duration> {
    fn is_valid(&self) -> bool;
    fn is_zero(&self) -> bool;
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
