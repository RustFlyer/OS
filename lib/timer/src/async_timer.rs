use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use crate::core::{TIMER_MANAGER, Timer};
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
            timer: Timer::new(timeout),
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
            this.timer.set_callback(cx.waker().clone());
            TIMER_MANAGER.add_timer(this.timer.clone());
            this.registered = true;
            log::debug!("[TimeoutFuture] Registered new timer");
        }

        Poll::Pending
    }
}

pub struct SleepFuture {
    timer: Timer,
    registered: bool,
}

impl SleepFuture {
    pub fn new(duration: Duration) -> Self {
        Self {
            timer: Timer::new(duration),
            registered: false,
        }
    }
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.timer.is_longer_than_expire(get_time_duration()) {
            return Poll::Ready(());
        }

        if !this.registered {
            this.timer.set_callback(cx.waker().clone());
            TIMER_MANAGER.add_timer(this.timer.clone());
            this.registered = true;
            log::debug!("[SleepFuture] Registered new timer");
        }

        Poll::Pending
    }
}

pub async fn sleep_ms(ms: u64) {
    SleepFuture::new(Duration::from_millis(ms)).await
}

pub async fn run_with_timeout<F: Future + Send + 'static>(
    timeout: Duration,
    future: F,
) -> TimedTaskResult<F::Output> {
    TimeoutFuture::new(timeout, future).await
}

pub struct IntervalFuture {
    timer: Timer,
    registered: bool,
}

impl IntervalFuture {
    pub fn new(period: Duration) -> Self {
        Self {
            timer: Timer::new_periodic(period, period),
            registered: false,
        }
    }
}

impl Future for IntervalFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if !this.registered {
            this.timer.set_callback(cx.waker().clone());
            TIMER_MANAGER.add_timer(this.timer.clone());
            this.registered = true;
            log::debug!("[IntervalFuture] Registered new periodic timer");
        }

        Poll::Pending
    }
}
