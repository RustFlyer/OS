#![no_std]
#![no_main]

extern crate alloc;
use core::usize;

use alloc::collections::VecDeque;
use async_task::{Runnable, ScheduleInfo, Task, WithInfo};
use lazy_static::lazy_static;
use mutex::SpinNoIrqLock;

use config::device::MAX_HARTS;

const HART_TASKS_LINE: TaskLine = TaskLine::new();
pub static mut HART_TASKS_LINES: [TaskLine; MAX_HARTS] = [HART_TASKS_LINE; MAX_HARTS];
pub static mut HART_RUN_MASK: usize = 0;

lazy_static! {
    static ref TASKLINE: TaskLine = TaskLine::new();
}

/// Task Line
///
/// Tasks in line is the runnable for async schedule.
/// Used to take control of async tasks.
struct TaskLine {
    tasks: SpinNoIrqLock<VecDeque<Runnable>>,
}

impl TaskLine {
    pub const fn new() -> Self {
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

#[allow(static_mut_refs)]
pub fn push_in_available_line(runnable: Runnable, info: ScheduleInfo) {
    // log::trace!("One Task is Waken!");
    let mut least_waiting_tasks_num: usize = usize::MAX;
    let mut available_line_id: usize = 0;
    for i in 0..MAX_HARTS {
        let hart_mask = 1 << i;
        unsafe {
            if (hart_mask & HART_RUN_MASK) != 0 {
                let waiting_num = HART_TASKS_LINES[i].length();
                if waiting_num < least_waiting_tasks_num {
                    least_waiting_tasks_num = waiting_num;
                    available_line_id = i;
                }
            }
        }
    }
    // log::debug!("push task into [{}] line", available_line_id);

    unsafe {
        if info.woken_while_running {
            HART_TASKS_LINES[available_line_id].push(runnable);
        } else {
            HART_TASKS_LINES[available_line_id].push_front(runnable);
        }
    }
}

pub fn spawn<F>(future: F) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let schedule = move |runnable: Runnable, info: ScheduleInfo| {
        // if info.woken_while_running {
        //     TASKLINE.push(runnable);
        // } else {
        //     TASKLINE.push_front(runnable);
        // }
        push_in_available_line(runnable, info);
    };
    async_task::spawn(future, WithInfo(schedule))
}

pub fn task_run_always_alone(hart_id: usize) {
    while let Some(task) = fetch_one(hart_id) {
        task.run();
    }
}

pub fn has_waiting_task_alone(hart_id: usize) -> bool {
    unsafe { HART_TASKS_LINES[hart_id].length() > 0 }
}

pub fn task_run_always() {
    while let Some(task) = TASKLINE.fetch() {
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

pub fn init(hart_id: usize) {
    unsafe {
        HART_RUN_MASK |= 1 << hart_id;
    }
}

pub fn fetch_one(hart_id: usize) -> Option<Runnable> {
    unsafe {
        if let Some(task) = HART_TASKS_LINES[hart_id].fetch() {
            return Some(task);
        }

        for i in 0..MAX_HARTS {
            if i == hart_id {
                continue;
            }
            let hart_mask = 1 << i;
            unsafe {
                if (hart_mask & HART_RUN_MASK) != 0 {
                    if let Some(task) = HART_TASKS_LINES[i].fetch() {
                        return Some(task);
                    }
                }
            }
        }
    }

    None
}
