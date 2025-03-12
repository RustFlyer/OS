use alloc::sync::Arc;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::task::task::TaskState;
use core::task::Waker;

use pps::ProcessorPrivilegeState;
pub struct UserFuture<F: Future + Send + 'static> {
    task: Arc<Task>,
    pps: ProcessorPrivilegeState,
    future: F,
}

impl<F: Future + Send + 'static> UserFuture<F> {
    pub fn new(task: Arc<Task>, future: F) -> Self {
        Self {
            task,
            pps: ProcessorPrivilegeState::new(),
            future,
        }
    }
}

impl<F: Future + Send + 'static> Future for UserFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
        // just user_switch_in
        // then poll future
        // then user_switch_out
    }
}

pub struct KernelFuture<F: Future + Send + 'static> {
    pps: ProcessorPrivilegeState,
    future: F,
}

impl<F: Future + Send + 'static> KernelFuture<F> {
    pub fn new(future: F) -> Self {
        Self {
            pps: ProcessorPrivilegeState::new(),
            future,
        }
    }
}

impl<F: Future + Send + 'static> Future for KernelFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
        // just kernel_switch_in
        // then poll future
        // then kernel_switch_out
    }
}

pub async fn task_executor_unit(task: Arc<Task>) {
    task.set_waker(take_waker().await);
    loop {
        todo!(); // trap_return_

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                todo!()
            }
            _ => {}
        }

        todo!(); // trap_handle_

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                todo!()
            }
            _ => {}
        }

        todo!(); // signal_handle_
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

// time schedule is coming soon....
