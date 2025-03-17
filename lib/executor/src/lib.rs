#![no_std]
#![no_main]

extern crate alloc;
use alloc::collections::VecDeque;
use async_task::{Runnable, ScheduleInfo, Task, WithInfo};
use mutex::SpinNoIrqLock;

use lazy_static::lazy_static;

lazy_static! {
    static ref TASKLINE: TaskLine = TaskLine::new();
}

/// 任务队列结构体
///
/// 异步调度的任务队列，与内核中TaskManager不同
/// 用来管理异步的任务
struct TaskLine {
    tasks: SpinNoIrqLock<VecDeque<Runnable>>,
}

impl TaskLine {
    pub fn new() -> Self {
        Self {
            tasks: SpinNoIrqLock::new(VecDeque::new()),
        }
    }
    pub fn push(&self, task: Runnable) {
        self.tasks.lock().push_back(task);
    }
    pub fn push_front(&self, task: Runnable) {
        self.tasks.lock().push_front(task);
    }
    pub fn fetch(&self) -> Option<Runnable> {
        self.tasks.lock().pop_front()
    }
    pub fn fetch_front(&self) -> Option<Runnable> {
        self.tasks.lock().pop_back()
    }
    pub fn length(&self) -> usize {
        self.tasks.lock().len()
    }
}

pub fn spawn<F>(future: F) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let schedule = move |runnable: Runnable, info: ScheduleInfo| {
        if info.woken_while_running {
            TASKLINE.push(runnable);
        } else {
            TASKLINE.push_front(runnable);
        }
    };
    async_task::spawn(future, WithInfo(schedule))
}

pub fn task_run_always() {
    while let Some(task) = TASKLINE.fetch() {
        log::debug!("try to fetch a task!");
        task.run();
    }
}

pub fn task_run_once() {
    if let Some(task) = TASKLINE.fetch() {
        task.run();
    }
}

pub fn task_run_once_front() {
    if let Some(task) = TASKLINE.fetch_front() {
        task.run();
    }
}

pub fn has_waiting_task() -> bool {
    TASKLINE.length() > 0
}
