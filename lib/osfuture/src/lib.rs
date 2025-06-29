#![no_std]
#![no_main]
extern crate alloc;

use alloc::{boxed::Box, sync::Arc, task::Wake};
use core::{
    pin::Pin,
    task::{Context, Poll, Waker, ready},
};

/// `take_waker()` gets future waker in current context. when future temps to
/// be waken, it can call this waker to wake it up and rejoin
/// the hart queue.
#[inline(always)]
pub async fn take_waker() -> Waker {
    TakeWakerFuture.await
}

/// `TakeWakerFuture` is created just to assist to implement
/// Future trait and get waker from its context.
struct TakeWakerFuture;

impl Future for TakeWakerFuture {
    type Output = Waker;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(cx.waker().clone())
    }
}

/// `SuspendFuture` relinquishes the execution of the current task.
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

/// the caller task will suspend and give cpu to other tasks.
/// If the task temps to rejoin the hart TaskLine again, its
/// waker should be called by other tasks.
pub async fn suspend_now() {
    log::debug!("suspend");
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

/// the caller task will suspend and give cpu to other tasks.
/// Unlike [`suspend_now()`], this function will call waker before
/// it is suspended. Therefore, it will join the end of the queue.
/// When the previous tasks are executed, it will run again.
pub async fn yield_now() {
    YieldFuture::new().await;
}

/// `BlockWaker` is a structure which implements Wake trait.
/// It means that this waker can be passed in context as a
/// waker and when it is called, it can act as its pre-set
/// action and not act by default.
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

/// Like [`block_on`], this function can wrap a async function in a sync function.
/// But different from `block_on`, it will loop for a certain number. If the async task can
/// return Ready during loop, it can return Ok. Otherwise, it will return Err.
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
                cnt += 1;
                if cnt > 10000 {
                    return Err(());
                }
                continue;
            }
        }
    }
}

pub enum SelectOutput<T1, T2> {
    Output1(T1),
    Output2(T2),
}

/// Select two futures at a time.
/// Note that future1 has a higher level than future2
pub struct Select2Futures<T1, T2, F1, F2>
where
    F1: Future<Output = T1>,
    F2: Future<Output = T2>,
{
    future1: F1,
    future2: F2,
}

impl<T1, T2, F1, F2> Select2Futures<T1, T2, F1, F2>
where
    F1: Future<Output = T1>,
    F2: Future<Output = T2>,
{
    pub fn new(future1: F1, future2: F2) -> Self {
        Self { future1, future2 }
    }
}

impl<T1, T2, F1, F2> Future for Select2Futures<T1, T2, F1, F2>
where
    F1: Future<Output = T1>,
    F2: Future<Output = T2>,
{
    type Output = SelectOutput<T1, T2>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let ret = unsafe { Pin::new_unchecked(&mut this.future1).poll(cx) };
        if ret.is_ready() {
            return Poll::Ready(SelectOutput::Output1(ready!(ret)));
        }
        let ret = unsafe { Pin::new_unchecked(&mut this.future2).poll(cx) };
        if ret.is_ready() {
            return Poll::Ready(SelectOutput::Output2(ready!(ret)));
        }
        Poll::Pending
    }
}
