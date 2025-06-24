use alloc::collections::BinaryHeap;
use core::{cmp::Reverse, time::Duration};

use mutex::SpinNoIrqLock;

use crate::{Timer, TimerState};

pub static TIMER_MANAGER: TimerManager = TimerManager::new();

pub struct TimerManager {
    timers: SpinNoIrqLock<BinaryHeap<Reverse<Timer>>>,
}

impl TimerManager {
    pub const fn new() -> Self {
        Self {
            timers: SpinNoIrqLock::new(BinaryHeap::new()),
        }
    }

    pub fn add_timer(&self, timer: Timer) {
        log::warn!("[TimerManager] add a new timer {:?}", timer);
        self.timers.lock().push(Reverse(timer));
    }

    pub fn check(&self, current: Duration) {
        let mut timers = self.timers.lock();
        while let Some(timer) = timers.peek() {
            if current < timer.0.expire || !timer.0.is_active() {
                break;
            }

            // log::error!(
            //     "[TimerManager] wake: current: {:?}, expire: {:?}",
            //     current,
            //     timer.0.expire
            // );
            let timer = timers.pop().unwrap().0;
            if let Some(event) = timer.clone().callback {
                if event.callback() != TimerState::Cancelled && timer.is_periodic() {
                    timers.push(Reverse(timer.another()));
                }
            }
        }
    }
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}
