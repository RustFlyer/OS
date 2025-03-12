#![no_std]
#![no_main]

mod core;
mod async_timer;

pub use core::{Timer, TimerManager, TimerState, TIMER_MANAGER};
pub use async_timer::{
    sleep_ms,
    run_with_timeout,
    TimedTaskResult,
    TimeoutFuture,
    SleepFuture,
    IntervalFuture,
};
