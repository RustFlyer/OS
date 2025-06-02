use alloc::sync::Arc;
use arch::time::{get_time_duration, set_nx_timer_irq};
use osfuture::{block_on_with_result, suspend_now, take_waker, yield_now};
use timer::TIMER_MANAGER;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::processor::hart::current_hart;
use crate::task::signal::sig_exec::sig_check;
use crate::task::task::TaskState;
use crate::trap;
use crate::trap::trap_syscall::async_syscall;

use pps::ProcessorPrivilegeState;

/// `UserFuture` is a schedule unit for a task in the kernel. It's built up
/// with user-task, env-processor privilege state and action-future.
///
/// User-task is the control unit of the thread/process. It stores the states
/// of all aspects, such as tid, task state, memory-space and so on.
/// Processor Privilege State is the environment of a user-future. Also, it's
/// the environment of a hart which is running the future. It stores sum_cnt, sstatus,
/// sepc and satp. When the user-future is hung up, the privilege state is stored.Then
/// the state is loaded as the user-future is scheduled.
/// Action-future refers to the main loop of a task, `task_executor_unit`.
///
/// `UserFuture` has implemented Future trait, which means it can be scheduled by
/// async-runtime.
pub struct UserFuture<F: Future + Send + 'static> {
    task: Arc<Task>,
    pps: ProcessorPrivilegeState,
    future: F,
}

/// UserFuture Impl provices `new` function to create a UserFuture.
/// Its pps(processor privilege state) will be initialized when the future
/// suspend or yield and then hart will store its states in pps during
/// its `user_switch_out`.
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

    /// When User Future is scheduled by executor, it will call this function.
    /// Hart calls user switch in to load the state and environment of user-future.
    /// Then user-future will call action-future and return to the address where it
    /// is left in its last schedule.
    ///
    /// When it finds task addrspace is locked(borrowed by other threads), it will try to
    /// wait for some time. If it can not get addrspace-lock after waiting, it will give up
    /// running and yield.
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

/// `KernelFuture` is also a schedule unit like `UserFuture`. But differrnt from
/// `UserFuture`, it does not consist of task.
///
/// `KernelFuture` controls simple events in the kernel. These tasks do not
/// require user memory-space, user-stack, tid and so on. Therefore, it can
/// directly spawn with impl Fn.
///
/// Attention: kernel future function should not be a loop without yield. Because
/// kernel future function does not rely on `task_executor_unit`, it does not
/// have timer and yield to others when it runs for a long time. The schedule of
/// KernelFuture is the duty of KernelFuture User. User should ensure running time of
/// the future.
pub struct KernelFuture<F: Future + Send + 'static> {
    pps: ProcessorPrivilegeState,
    future: F,
}

/// KernelFuture Impl provices `new` function to create a KernelFuture.
/// Its pps(processor privilege state) will be initialized when the future
/// suspend or yield and then hart will store its states in pps during
/// its `kernel_switch_out`.
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

    /// Compared with UserFuture poll, KernelFuture poll does not switch user addrspace
    /// and not have record timer. It can be implemented more simplily without considering
    /// async problems.
    ///
    /// In other aspects, KernelFuture poll performs similarly to UserFuture. When future is polled
    /// again, kernel will enter from this function and continue in the position where it last left.  
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let future = unsafe { Pin::get_unchecked_mut(self) };
        let hart = current_hart();
        hart.kernel_switch_in(&mut future.pps);
        let ret = unsafe { Pin::new_unchecked(&mut future.future).poll(cx) };
        hart.kernel_switch_out(&mut future.pps);
        ret
    }
}

/// `task_executor_unit` is a basic user unit used to schedule by the global executor.
/// When a `UserFuture` is spawned by `spawn_user_task`, it will catch a new `task` and
/// become a UserFuture stored in executor's TaskLine.
///
/// When global executor executes a UserFuture, task will catch the waker of future from
/// its context in the first time that `task_executor_unit` is executed. The Waker is used
/// to wake the future as it's sleeping which means that the UserFuture is not in the
/// TaskLine.
///
/// Then the program counter will advance in and get into a loop of user task. The loop mainly
/// consists of trap_return, trap_handle and sig_check. The application returns to the user
/// space and execute user instructions through trap_return. When trapped in user space, it returns
/// to kernel space through trap_return and handle exceptional traps by trap_handle. If there are
/// some signals received by the thread, thread can handle them in the sig_check.
///
/// Finally, the application will break from loop when it is set as Zombie. Then it will call
/// exit() and wait parent process to recycle it and clean remained rubbish.
pub async fn task_executor_unit(task: Arc<Task>) {
    log::debug!(
        "hart {}: run task [{}] in first time!",
        current_hart().id,
        task.get_name()
    );
    task.set_waker(take_waker().await);
    set_nx_timer_irq();

    (task.tid() == 4).then(simdebug::stop);

    loop {
        // trap_return connects user and kernel.
        trap::trap_return(&task);

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Sleeping => {
                suspend_now().await;
            }
            _ => {}
        }

        // handle user trap, not for kernel trap. Therefore, there should not
        // be some instructions with risks between trap_return and trap_handle.
        trap::trap_handler(&task);

        let mut interrupted = async_syscall(&task).await;

        TIMER_MANAGER.check(get_time_duration());

        if task.is_yield() {
            yield_now().await;
            task.set_is_yield(false);
        }

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Sleeping => {
                task.set_state(TaskState::Interruptable);
                suspend_now().await;
            }
            _ => {}
        }

        // param "intr" always false for now
        sig_check(task.clone(), &mut interrupted).await;

        // threads may be killed or stopped in sig_check.
        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Sleeping => {
                task.set_state(TaskState::Interruptable);
                suspend_now().await;
            }
            _ => {}
        }
    }

    task.exit();
}

/// `spawn_user_task` wraps task control unit and create a schedule unit by `task_executor_unit`
/// to spawn a UserFuture for executor to schedule.
///
/// When a task is initialized, it can be passed into this function and spawn a future
/// to schedule.
pub fn spawn_user_task(task: Arc<Task>) {
    let future = UserFuture::new(task.clone(), task_executor_unit(task));
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

/// `spawn_kernel_task` wraps a future(impl async Fn) and spawn a KernelFuture.
///
/// It can be used for creating some simple or single kernel tasks such as a timer
/// update kernel thread.
///
/// Attention: `future` should implement yield at regular intervals(or by certain step).
/// Otherwise, this `future` will occupy the cpu without giving it up.
pub fn spawn_kernel_task<F: Future<Output = ()> + Send + 'static>(future: F) {
    let future = KernelFuture::new(future);
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}
