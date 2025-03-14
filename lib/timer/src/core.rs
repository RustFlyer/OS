extern crate alloc;

use alloc::collections::BinaryHeap;
use arch::riscv64::time::get_time_duration;
use core::cmp::Reverse;
use core::task::Waker;
use core::time::Duration;
use mutex::SpinNoIrqLock;
use spin::Lazy;

/// 定时器状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Active,
    Expired,
    Cancelled,
}

/// 一个带有回调功能的定时器
#[derive(Debug, Clone)]
pub struct Timer {
    expire: Duration,
    callback: Option<Waker>,
    state: TimerState,
    periodic: bool,
    period: Option<Duration>,
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

    fn reset_periodic(&mut self) {
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

pub struct TimerManager {
    timers: SpinNoIrqLock<BinaryHeap<Reverse<Timer>>>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            timers: SpinNoIrqLock::new(BinaryHeap::new()),
        }
    }

    pub fn add_timer(&self, timer: Timer) {
        self.timers.lock().push(Reverse(timer));
    }

    pub fn check(&self, current: Duration) {
        let mut timers = self.timers.lock();
        while let Some(Reverse(mut timer)) = timers.peek().cloned() {
            if current < timer.expire || !timer.is_active() {
                break;
            }

            // Remove the timer from heap
            timers.pop();

            // Handle periodic timer
            if timer.is_periodic() {
                timer.reset_periodic();
                timers.push(Reverse(timer.clone()));
            } else {
                timer.state = TimerState::Expired;
            }

            // Wake up the task
            if let Some(waker) = timer.callback.take() {
                log::debug!(
                    "[Timer Manager] Timer expired at {:?}, scheduled for {:?}",
                    current,
                    timer.expire
                );
                waker.wake();
            }
        }
    }
}

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);
