use alloc::sync::Arc;
use osfuture::{block_on, block_on_with_result};

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::processor::hart::current_hart;
use crate::task::signal::sig_exec::sig_check;
use crate::task::task::TaskState;
use crate::trap;
use core::task::Waker;

use pps::ProcessorPrivilegeState;

/// UserFuture
///
/// Wrap user tasks and their associated futures to manage privilege state switching
pub struct UserFuture<F: Future + Send + 'static> {
    task: Arc<Task>,
    pps: ProcessorPrivilegeState,
    future: F,
}

impl<F: Future + Send + 'static> UserFuture<F> {
    #[inline]
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
        let future = unsafe { Pin::get_unchecked_mut(self) };
        let hart = current_hart();
        let r = block_on_with_result(async {
            hart.user_switch_in(&mut future.task, &mut future.pps).await
        });

        let ret = if r.is_ok() {
            unsafe { Pin::new_unchecked(&mut future.future).poll(cx) }
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };

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
        let future = unsafe { Pin::get_unchecked_mut(self) };
        let hart = current_hart();
        hart.kernel_switch_in(&mut future.pps);
        let ret = unsafe { Pin::new_unchecked(&mut future.future).poll(cx) };
        hart.kernel_switch_out(&mut future.pps);
        ret
    }
}

/// Top-Level of Task
///
/// Task will run in this loop in the kernel all the time
pub async fn task_executor_unit(task: Arc<Task>) {
    log::debug!(
        "hart {}: run task [{}] in first time!",
        current_hart().id,
        task.get_name()
    );
    task.set_waker(take_waker().await);

    loop {
        // log::debug!("try to step into user!");

        trap::trap_return(&task);

        // log::debug!("return from user!");

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                suspend_now().await;
            }
            _ => {}
        }

        trap::trap_handler(&task).await;

        let id = current_hart().id;
        if task.timer_mut().schedule_time_out() && executor::has_waiting_task_alone(id) {
            yield_now().await;
        }

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                suspend_now().await;
            }
            _ => {}
        }

        // param "intr" always false for now
        sig_check(task.clone(), false).await;
    }

    task.exit();
}

/// spawn user task
///
/// Wrap the user task as a UserFuture and submit it to the scheduler
pub fn spawn_user_task(task: Arc<Task>) {
    log::info!("New Task [{}] spawns!", task.get_name());
    let future = UserFuture::new(task.clone(), task_executor_unit(task));
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

/// spawn kernel task
///
/// Wrap the Future as a KernelFuture and submit it to the scheduler
pub fn spawn_kernel_task<F: Future<Output = ()> + Send + 'static>(future: F) {
    let future = KernelFuture::new(future);
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

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
