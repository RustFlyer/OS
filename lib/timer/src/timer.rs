use alloc::sync::Arc;
use arch::time::get_time_duration;
use core::task::Waker;
use core::time::Duration;

use crate::event::{IEvent, WakerEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Active,
    Expired,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct Timer {
    pub expire: Duration,
    pub callback: Option<Arc<dyn IEvent>>,
    pub state: TimerState,
    pub periodic: bool,
    pub period: Option<Duration>,
    pub signal: Option<(i32, usize)>,
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

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Self {
            expire: duration,
            callback: None,
            state: TimerState::Active,
            periodic: false,
            period: None,
            signal: None,
        }
    }

    pub fn new_periodic(duration: Duration, period: Duration) -> Self {
        Self {
            expire: get_time_duration() + duration,
            callback: None,
            state: TimerState::Active,
            periodic: true,
            period: Some(period),
            signal: None,
        }
    }

    pub fn set_waker_callback(&mut self, waker: Waker) {
        self.callback = Some(Arc::new(WakerEvent { waker }));
    }

    pub fn set_callback(&mut self, event: Arc<dyn IEvent>) {
        self.callback = Some(event);
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

    pub fn is_longer_than_expire(&self, duration: Duration) -> bool {
        self.expire <= duration
    }

    pub fn another(&self) -> Self {
        assert!(self.is_periodic());
        let period = self.period.unwrap();
        let mut timer = Timer::new_periodic(period, period);
        if let Some(callback) = &self.callback {
            timer.set_callback((*callback).clone());
        }
        timer
    }
}
