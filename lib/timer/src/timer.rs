use arch::riscv64::time::get_time_duration;
use core::task::Waker;
use core::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Active,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub(crate) expire: Duration,
    pub(crate) callback: Option<Waker>,
    pub(crate) state: TimerState,
    pub(crate) periodic: bool,
    pub(crate) period: Option<Duration>,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Self {
            expire: get_time_duration() + duration,
            callback: None,
            state: TimerState::Active,
            periodic: false,
            period: None,
        }
    }

    pub fn new_periodic(duration: Duration, period: Duration) -> Self {
        Self {
            expire: get_time_duration() + duration,
            callback: None,
            state: TimerState::Active,
            periodic: true,
            period: Some(period),
        }
    }

    pub fn set_callback(&mut self, waker: Waker) {
        self.callback = Some(waker);
    }

    pub fn cancel(&mut self) {
        self.state = TimerState::Cancelled;
    }

    pub fn is_active(&self) -> bool {
        self.state == TimerState::Active
    }

    pub fn is_periodic(&self) -> bool {
        self.periodic
    }

    pub(crate) fn reset_periodic(&mut self) {
        if let Some(period) = self.period {
            self.expire += period;
            self.state = TimerState::Active;
        }
    }

    pub fn is_longer_than_expire(&self, duration: Duration) -> bool {
        self.expire <= duration
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.expire.cmp(&other.expire)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Timer {}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expire == other.expire
    }
}
