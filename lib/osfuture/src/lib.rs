#![no_std]
#![no_main]

use core::{
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// Gets the current context waker  
#[inline(always)]
pub async fn take_waker() -> Waker {
    TakeWakerFuture.await
}

/// Take Waker Future
///
/// Returns a Waker clone of the current context directly on the first poll
struct TakeWakerFuture;

impl Future for TakeWakerFuture {
    type Output = Waker;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(cx.waker().clone())
    }
}

/// Suspend Future
///
/// Relinquishes the execution of the current task
struct SuspendFuture {
    has_suspended: bool,
}

impl SuspendFuture {
    const fn new() -> Self {
        Self {
            has_suspended: false,
        }
    }
}

impl Future for SuspendFuture {
    type Output = ();

    /// Suspend logic:
    /// - The first poll returns Pending (triggers pending)
    /// - Subsequent polls return to Ready (Resume Execution)
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        match self.has_suspended {
            true => Poll::Ready(()),
            false => {
                self.has_suspended = true;
                Poll::Pending
            }
        }
    }
}

/// Suspend the current task Immediately
///
/// With the await function, the current task will be relinquished
/// to the processor until it is scheduled again
pub async fn suspend_now() {
    SuspendFuture::new().await
}

struct YieldFuture {
    has_yielded: bool,
}

impl YieldFuture {
    const fn new() -> Self {
        Self { has_yielded: false }
    }
}

impl Future for YieldFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.has_yielded {
            true => Poll::Ready(()),
            false => {
                self.has_yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

pub async fn yield_now() {
    YieldFuture::new().await;
}
