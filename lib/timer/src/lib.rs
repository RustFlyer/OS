#![no_std]
#![no_main]

mod async_timer;
mod core;

pub use async_timer::{
    IntervalFuture, SleepFuture, TimedTaskResult, TimeoutFuture, run_with_timeout, sleep_ms,
};
pub use core::{TIMER_MANAGER, Timer, TimerManager, TimerState};
