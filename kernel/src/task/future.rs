use alloc::sync::Arc;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::task::task::TaskState;
use core::task::Waker;

#[derive(Debug, Clone, Copy)]
pub struct FutureContext {}

impl FutureContext {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct UserFuture<F: Future + Send + 'static> {
    task: Arc<Task>,
    context: FutureContext,
    future: F,
}

impl<F: Future + Send + 'static> UserFuture<F> {
    pub fn new(task: Arc<Task>, future: F) -> Self {
        Self {
            task,
            context: FutureContext::new(),
            future,
        }
    }
}

impl<F: Future + Send + 'static> Future for UserFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}

pub struct KernelFuture<F: Future + Send + 'static> {
    context: FutureContext,
    future: F,
}

impl<F: Future + Send + 'static> KernelFuture<F> {
    pub fn new(future: F) -> Self {
        Self {
            context: FutureContext {},
            future,
        }
    }
}

impl<F: Future + Send + 'static> Future for KernelFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}

pub async fn task_executor_unit(task: Arc<Task>) {
    task.set_waker(take_waker().await);
    loop {
        todo!();

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                todo!()
            }
            _ => {}
        }

        todo!();

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                todo!()
            }
            _ => {}
        }

        todo!();
    }

    task.exit();
}

pub fn spawn_user_task(task: Arc<Task>) {
    let future = UserFuture::new(task.clone(), task_executor_unit(task));
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

pub fn spawn_kernel_task<F: Future<Output = ()> + Send + 'static>(future: F) {
    let future = KernelFuture::new(future);
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

/// Take the waker of the current future
#[inline(always)]
pub async fn take_waker() -> Waker {
    TakeWakerFuture.await
}

struct TakeWakerFuture;

impl Future for TakeWakerFuture {
    type Output = Waker;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(cx.waker().clone())
    }
}
