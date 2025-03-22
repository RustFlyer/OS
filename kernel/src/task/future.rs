use alloc::sync::Arc;
use simdebug::when_debug;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::Task;
use crate::processor::hart::{current_hart, one_hart};
use crate::task::task::TaskState;
use crate::trap;
use core::task::Waker;

use pps::ProcessorPrivilegeState;

/// 用户态Future封装结构
///
/// 包装用户任务及其关联的Future，管理特权状态切换
/// 泛型F需满足Send + 'static保证线程安全和静态生命周期
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

    /// 核心poll实现
    ///
    /// 1. 切换到用户态上下文
    /// 2. 执行实际Future的poll
    /// 3. 切换回内核态上下文
    ///
    /// 安全性：Pin保证整个结构体在内存中固定，因此可以安全获取内部字段的可变引用
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut future = unsafe { Pin::get_unchecked_mut(self) };
        let mut hart = current_hart();
        hart.user_switch_in(&mut future.task, &mut future.pps);
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
        let mut hart = current_hart();
        hart.kernel_switch_in(&mut future.pps);
        let ret = unsafe { Pin::new_unchecked(&mut future.future).poll(cx) };
        hart.kernel_switch_out(&mut future.pps);
        ret
    }
}

/// 任务执行单元异步函数
///
/// 这是任务执行的顶层循环，处理任务状态转换和事件处理
pub async fn task_executor_unit(task: Arc<Task>) {
    log::debug!("run task in first time!");
    // 设置任务的唤醒器（从当前上下文中获取）
    task.set_waker(take_waker().await);
    loop {
        // log::debug!("try to step into user!");

        trap::trap_return(&task); // trap_return_

        // log::debug!("return from user!");

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                suspend_now().await;
            }
            _ => {}
        }

        trap::trap_handler(&task).await; // trap_handle_

        match task.get_state() {
            TaskState::Zombie => break,
            TaskState::Waiting => {
                suspend_now().await;
            }
            _ => {}
        }

        // signal_handle_
    }

    task.exit();
}

/// 生成用户任务
///
/// 将用户任务包装为UserFuture并提交给调度器
pub fn spawn_user_task(task: Arc<Task>) {
    let future = UserFuture::new(task.clone(), task_executor_unit(task));
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach();
}

/// 生成内核任务
///
/// 将内核Future包装为KernelFuture并提交给调度器
pub fn spawn_kernel_task<F: Future<Output = ()> + Send + 'static>(future: F) {
    let future = KernelFuture::new(future);
    let (task, handle) = executor::spawn(future);
    task.schedule();
    handle.detach(); // 分离执行句柄（不等待结果）
}

/// 异步获取当前上下文唤醒器
#[inline(always)]
pub async fn take_waker() -> Waker {
    TakeWakerFuture.await
}

/// 实现Waker获取的Future
///
/// 在首次poll时直接返回当前上下文的Waker克隆
struct TakeWakerFuture;

impl Future for TakeWakerFuture {
    type Output = Waker;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 直接返回当前上下文的Waker克隆
        Poll::Ready(cx.waker().clone())
    }
}

/// 挂起Future实现
///
/// 用于实现suspend_now功能，使当前任务让出执行权
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

    /// 挂起逻辑实现：
    /// - 第一次poll返回Pending（触发挂起）
    /// - 后续poll返回Ready（恢复执行）
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

/// 立即挂起当前任务
///
/// 通过await这个异步函数，当前任务会让出处理器直到被再次调度
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
