use crate::task::tid::{Tid, TidHandle, tid_alloc};

extern crate alloc;
use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use mutex::SpinNoIrqLock;

use core::{cell::SyncUnsafeCell, num::IntErrorKind};
use core::{ops::DerefMut, task::Waker};

use mutex::UPSafeCell;
use time::TaskTimeStat;
use timer::{TIMER_MANAGER, Timer};

use arch::riscv64::time::get_time_duration;

use core::time::Duration;

use super::{
    future::{self, spawn_user_task},
    manager::add_task,
};

/// 任务状态枚举
///
/// 表示一个任务的状态，包括运行、睡眠、等待和僵死
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    Running,
    Sleeping,
    Waiting,
    Zombie,
}

/// 任务结构体
///
/// 表示一个任务，包含任务ID、进程关系、状态和内部状态
pub struct Task {
    tid: TidHandle,
    process: Option<Weak<Task>>,
    is_process: bool,

    inner: UPSafeCell<TaskInner>,
}

/// 任务内部状态结构体
///
/// 表示一个任务的内部状态，包含任务状态、父任务、子任务、唤醒器和计时器
pub struct TaskInner {
    state: TaskState,
    parent: Option<Weak<Task>>,
    children: BTreeMap<Tid, Weak<Task>>,
    waker: Option<Waker>,
    timer: TaskTimeStat,

    exit_code: u32,
}

impl Task {
    pub fn tid(&self) -> Tid {
        self.tid.0
    }

    pub fn process(self: &Arc<Self>) -> Arc<Task> {
        if self.is_process() {
            self.process.as_ref().cloned().unwrap().upgrade().unwrap()
        } else {
            self.clone()
        }
    }

    pub fn is_process(&self) -> bool {
        self.is_process
    }

    pub fn is_in_state(&mut self, state: TaskState) -> bool {
        self.inner.exclusive_access().is_in_state(state)
    }

    pub fn set_state(&mut self, state: TaskState) {
        self.inner.exclusive_access().set_state(state);
    }

    pub fn get_state(&mut self) -> TaskState {
        self.inner.exclusive_access().get_state()
    }

    pub fn set_parent(&mut self, parent: Arc<Task>) {
        self.inner.exclusive_access().parent = Some(Arc::downgrade(&parent));
    }

    pub fn add_child(&mut self, child: Arc<Task>) {
        self.inner
            .exclusive_access()
            .children
            .insert(child.tid(), Arc::downgrade(&child));
    }

    pub fn remove_child(&mut self, child: Arc<Task>) {
        self.inner.exclusive_access().children.remove(&child.tid());
    }

    pub fn new() -> Self {
        let inner = TaskInner::new();
        Task {
            tid: tid_alloc(),
            process: None,
            is_process: false,
            inner: unsafe { UPSafeCell::new(inner) },
        }
    }

    pub fn set_exit_code(&self, exit_code: u32) {
        self.inner.exclusive_access().set_exit_code(exit_code);
    }

    pub fn set_waker(&self, waker: Waker) {
        self.inner.exclusive_access().set_waker(waker);
    }

    pub fn get_waker(&self) -> Waker {
        self.inner.exclusive_access().get_waker().clone()
    }

    pub fn get_exit_code(&self) -> u32 {
        self.inner.exclusive_access().exit_code
    }

    pub fn exit(&mut self) {
        self.inner.exclusive_access().clear();
    }

    pub fn clear(&mut self) {
        self.inner.exclusive_access().clear();
    }

    pub fn enter_user_mode(&mut self) {
        self.inner.exclusive_access().enter_user_mode();
    }

    pub fn enter_kernel_mode(&mut self) {
        self.inner.exclusive_access().enter_kernel_mode();
    }

    pub fn spawn_task(elf_data: &[u8]) {
        let memory_set = todo!();
        let trap_context = todo!();
        let task_inner = todo!();
        let task = Arc::new(Task::new());
        add_task(&task);
        spawn_user_task(task);
    }
}

impl TaskInner {
    pub fn new() -> Self {
        Self {
            state: TaskState::Running,
            parent: None,
            children: BTreeMap::new(),
            waker: None,
            exit_code: 0,

            timer: TaskTimeStat::new(),
        }
    }

    pub fn enter_user_mode(&mut self) {
        self.timer.switch_to_user();
    }

    pub fn enter_kernel_mode(&mut self) {
        self.timer.switch_to_kernel();
    }

    pub fn clear(&mut self) {
        self.state = TaskState::Zombie;
        self.exit_code = 0;
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }

    pub fn get_waker(&self) -> &Waker {
        self.waker.as_ref().unwrap()
    }

    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }

    pub fn get_state(&self) -> TaskState {
        self.state
    }

    pub fn set_exit_code(&mut self, exit_code: u32) {
        self.exit_code = exit_code;
    }

    pub fn is_in_state(&self, state: TaskState) -> bool {
        self.state == state
    }

    pub async fn suspend_timeout(&self, limit: Duration) -> Duration {
        let expire = get_time_duration() + limit;
        let mut timer = Timer::new(expire);
        timer.set_callback(self.get_waker().clone());
        TIMER_MANAGER.add_timer(timer);
        future::suspend_now().await;
        let now = get_time_duration();
        if expire > now {
            expire - now
        } else {
            Duration::ZERO
        }
    }
}
