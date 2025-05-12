#![no_std]
#![no_main]
extern crate alloc;

mod async_timer;
mod event;
mod timer;
mod timer_manager;

use core::time::Duration;

use arch::riscv64::time::get_time_duration;
pub use async_timer::{TimedTaskResult, TimeoutFuture, run_with_timeout};
pub use event::IEvent;
use osfuture::take_waker;
pub use timer::{Timer, TimerState};
pub use timer_manager::{TIMER_MANAGER, TimerManager};

pub async fn sleep_ms(ms: usize) -> Duration {
    let limit: Duration = Duration::from_micros(ms as u64);
    let expire = get_time_duration() + limit;
    let mut timer = Timer::new(expire);
    timer.set_waker_callback(take_waker().await);
    TIMER_MANAGER.add_timer(timer);
    osfuture::suspend_now().await;
    let now = get_time_duration();
    if expire > now {
        expire - now
    } else {
        Duration::ZERO
    }
}
