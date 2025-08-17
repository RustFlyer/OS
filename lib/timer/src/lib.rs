#![no_std]
#![no_main]
extern crate alloc;

mod async_timer;
mod event;
mod timer;
mod timer_manager;

use arch::time::get_time_duration;
pub use async_timer::{TimedTaskResult, TimeoutFuture, run_with_timeout};
use core::time::Duration;
pub use event::IEvent;
use osfuture::take_waker;
pub use timer::{Timer, TimerState};
pub use timer_manager::{TIMER_MANAGER, TimerManager};

pub async fn sleep_ms(ms: usize) -> Duration {
    let limit: Duration = Duration::from_micros(ms as u64);
    let current_time = get_time_duration();
    
    // Check for potential overflow when adding limit to current time
    let expire = if let Some(exp) = current_time.checked_add(limit) {
        exp
    } else {
        // If overflow would occur, cap at Duration::MAX to avoid panic
        log::warn!("[sleep_ms] Duration overflow prevented, using Duration::MAX");
        Duration::MAX
    };
    
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
