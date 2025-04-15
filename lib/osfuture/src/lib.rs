#![no_std]
#![no_main]
extern crate alloc;

use alloc::{boxed::Box, sync::Arc, task::Wake};
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

/// A waker that wakes up the current thread when called.
struct BlockWaker;

impl Wake for BlockWaker {
    fn wake(self: Arc<Self>) {
        log::trace!("block waker wakes");
    }
}

/// Run a future to completion on the current thread.
/// Note that since this function is used in kernel mode,
/// we won't switch thread when the inner future pending.
/// Instead, we just poll the inner future again and again.
pub fn block_on<T>(fut: impl Future<Output = T>) -> T {
    // Pin the future so it can be polled.
    let mut fut = Box::pin(fut);

    let waker = Arc::new(BlockWaker).into();
    let mut cx = Context::from_waker(&waker);

    // Run the future to completion.
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => return res,
            Poll::Pending => continue,
        }
    }
}

pub fn block_on_with_result<T>(fut: impl Future<Output = T>) -> Result<T, ()> {
    // Pin the future so it can be polled.
    let mut fut = Box::pin(fut);
    let mut cnt = 0;

    let waker = Arc::new(BlockWaker).into();
    let mut cx = Context::from_waker(&waker);

    // Run the future to completion.
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => return Ok(res),
            Poll::Pending => {
                cnt = cnt + 1;
                if cnt > 10000 {
                    return Err(());
                }
                continue;
            }
        }
    }
}
