#![no_std]
#![no_main]
extern crate alloc;

mod async_timer;
mod event;
mod timer;
mod timer_manager;

pub use async_timer::{TimedTaskResult, TimeoutFuture, run_with_timeout};
pub use event::IEvent;
pub use timer::{Timer, TimerState};
pub use timer_manager::{TIMER_MANAGER, TimerManager};
