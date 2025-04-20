use core::{cmp::Reverse, time::Duration};

use alloc::collections::BinaryHeap;
use mutex::SpinNoIrqLock;
use spin::lazy::Lazy;

use crate::{Timer, TimerState};

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);

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

            timers.pop();
            if timer.is_periodic() {
                timer.reset_periodic();
                timers.push(Reverse(timer.clone()));
            } else {
                timer.state = TimerState::Expired;
            }

            if let Some(waker) = timer.callback.take() {
                waker.wake();
            }
        }
    }
}
