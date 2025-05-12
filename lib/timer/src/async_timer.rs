use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use crate::{timer::Timer, timer_manager::TIMER_MANAGER};
use arch::riscv64::time::get_time_duration;

#[derive(Debug)]
pub enum TimedTaskResult<T> {
    Completed(T),
    Timeout,
}

pub struct TimeoutFuture<F: Future + Send + 'static> {
    future: F,
    timer: Timer,
    registered: bool,
}

impl<F: Future + Send + 'static> TimeoutFuture<F> {
    pub fn new(timeout: Duration, future: F) -> Self {
        Self {
            future,
            timer: Timer::new(timeout + get_time_duration()),
            registered: false,
        }
    }
}

impl<F: Future + Send + 'static> Future for TimeoutFuture<F> {
    type Output = TimedTaskResult<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        // First, try polling the inner future
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        if let Poll::Ready(output) = future.poll(cx) {
            return Poll::Ready(TimedTaskResult::Completed(output));
        }

        // Check if we've timed out
        if this.timer.is_longer_than_expire(get_time_duration()) {
            return Poll::Ready(TimedTaskResult::Timeout);
        }

        // Register the timer if we haven't already
        if !this.registered {
            this.timer.set_waker_callback(cx.waker().clone());
            TIMER_MANAGER.add_timer(this.timer.clone());
            this.registered = true;
            log::debug!("[TimeoutFuture] Registered new timer");
        }

        Poll::Pending
    }
}

pub async fn run_with_timeout<F: Future + Send + 'static>(
    timeout: Duration,
    future: F,
) -> TimedTaskResult<F::Output> {
    TimeoutFuture::new(timeout, future).await
}
