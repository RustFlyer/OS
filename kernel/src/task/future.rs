use alloc::sync::Arc;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::processor::hart::current_hart;
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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut future = unsafe { Pin::get_unchecked_mut(self) };
        let hart = current_hart();
        hart.user_switch_in(future.task.clone(), &mut future.pps);
        let ret = unsafe { Pin::new_unchecked(&mut future.future).poll(cx) };
        hart.user_switch_out(&mut future.pps);
        ret
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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut future = unsafe { Pin::get_unchecked_mut(self) };
        let hart = current_hart();
        hart.kernel_switch_in(&mut future.pps);
        let ret = unsafe { Pin::new_unchecked(&mut future.future).poll(cx) };
        hart.kernel_switch_out(&mut future.pps);
        ret
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

// 挂起当前线程
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

pub async fn suspend_now() {
    SuspendFuture::new().await
}
