#![no_std]
#![no_main]
extern crate alloc;

mod async_timer;
mod timer;
mod timer_manager;

pub use async_timer::{
    IntervalFuture, SleepFuture, TimedTaskResult, TimeoutFuture, run_with_timeout, sleep_ms,
};
pub use timer::{Timer, TimerState};
pub use timer_manager::{TIMER_MANAGER, TimerManager};
