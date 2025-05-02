use core::time::Duration;

use crate::{TimeVal, TimeValue};

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ITimerVal {
    /// Interval for periodic timer
    pub it_interval: TimeVal,
    /// time until next expired
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

#[derive(Default, Debug, Clone, Copy)]
pub struct ITimer {
    pub interval: Duration,
    pub next_expire: Duration,
    pub id: usize,
}
