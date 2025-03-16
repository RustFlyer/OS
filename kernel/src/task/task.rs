use crate::{
    task::tid::{Tid, TidHandle, tid_alloc},
    trap,
};

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

use crate::trap::trap_context::TrapContext;
use crate::vm::addr_space::AddrSpace;
use crate::vm::addr_space::switch_to;
use crate::vm::elf::load_elf;

use arch::riscv64::time::get_time_duration;

use core::time::Duration;

use super::{
    future::{self, spawn_user_task},
    manager::add_task,
    tid::Pid,
};

/// 任务状态枚举
///
/// 表示一个任务的状态，包括运行、睡眠、等待和僵死
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    Running,
    Zombie,
    Waiting,
    Sleeping,
    Interruptable,
    UnInterruptable,
}

/// 任务结构体
///
/// 表示一个任务，包含任务ID、进程关系、状态和内部状态
pub struct Task {
    tid: TidHandle,
    process: Option<Weak<Task>>,
    is_process: bool,

    trap_context: SyncUnsafeCell<TrapContext>,
    timer: SyncUnsafeCell<TaskTimeStat>,
    waker: SyncUnsafeCell<Option<Waker>>,
    state: SpinNoIrqLock<TaskState>,
    addr_space: SpinNoIrqLock<AddrSpace>,
    inner: UPSafeCell<TaskInner>,
}

/// 任务内部状态结构体
///
/// 表示一个任务的内部状态，包含任务状态、父任务、子任务和退出代码
pub struct TaskInner {
    parent: Option<Weak<Task>>,
    children: BTreeMap<Tid, Weak<Task>>,

    exit_code: u32,
}

impl Task {
    pub fn tid(&self) -> Tid {
        self.tid.0
    }

    pub fn pid(self: &Arc<Self>) -> Pid {
        self.process().tid()
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

    pub fn is_in_state(&self, state: TaskState) -> bool {
        self.get_state() == state
    }

    pub fn get_state(&self) -> TaskState {
        self.state.lock().clone()
    }

    pub fn set_state(&self, state: TaskState) {
        *self.state.lock() = state;
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

    pub fn trap_context_mut(&self) -> &mut TrapContext {
        unsafe { &mut *self.trap_context.get() }
    }

    pub fn timer_mut(&self) -> &mut TaskTimeStat {
        unsafe { &mut *self.timer.get() }
    }

    pub fn waker_mut(&self) -> &mut Option<Waker> {
        unsafe { &mut *self.waker.get() }
    }

    pub fn addr_space_mut(&self) -> &SpinNoIrqLock<AddrSpace> {
        &self.addr_space
    }

    pub fn new() -> Self {
        let inner = TaskInner::new();
        Task {
            tid: tid_alloc(),
            process: None,
            is_process: false,
            trap_context: unsafe { SyncUnsafeCell::new(TrapContext::new(0, 0)) },
            timer: unsafe { SyncUnsafeCell::new(TaskTimeStat::new()) },
            waker: unsafe { SyncUnsafeCell::new(None) },
            state: unsafe { SpinNoIrqLock::new(TaskState::Waiting) },
            addr_space: SpinNoIrqLock::new(AddrSpace::build_user().unwrap()),
            inner: unsafe { UPSafeCell::new(inner) },
        }
    }

    pub fn set_exit_code(&self, exit_code: u32) {
        self.inner.exclusive_access().set_exit_code(exit_code);
    }

    pub fn set_waker(&self, waker: Waker) {
        unsafe { *self.waker_mut() = Some(waker) };
    }

    pub fn get_waker(&self) -> Waker {
        unsafe { self.waker_mut().as_ref().unwrap().clone() }
    }

    pub fn get_exit_code(&self) -> u32 {
        self.inner.exclusive_access().exit_code
    }

    pub fn exit(&self) {
        self.inner.exclusive_access().clear();
    }

    pub fn clear(&self) {
        self.inner.exclusive_access().clear();
    }

    pub fn inner_mut(&mut self) -> &mut UPSafeCell<TaskInner> {
        &mut self.inner
    }

    /// 当前任务切换到用户模式
    pub fn enter_user_mode(&mut self) {
        self.timer_mut().switch_to_user();
    }

    /// 当前任务切换到内核模式
    pub fn enter_kernel_mode(&mut self) {
        self.timer_mut().switch_to_kernel();
    }

    // 挂起当前任务，等待时间到达或被唤醒
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

    pub fn spawn_from_elf(&self, elf_data: &'static [u8]) {
        let mut addspace = self.addr_space.lock();
        let entry_point = load_elf(&mut addspace, elf_data);
    }

    pub fn switch_pagetable(&self, old_space: &AddrSpace) {
        switch_to(old_space, &self.addr_space.lock());
    }
}

impl TaskInner {
    pub fn new() -> Self {
        Self {
            parent: None,
            children: BTreeMap::new(),
            exit_code: 0,
        }
    }

    pub fn clear(&mut self) {
        self.exit_code = 0;
    }

    pub fn set_exit_code(&mut self, exit_code: u32) {
        self.exit_code = exit_code;
    }
}

pub fn spawn_task(elf_data: &'static [u8]) {
    let mut addr_space = AddrSpace::build_user().unwrap();
    let entry_point = addr_space.load_elf(elf_data).unwrap();
    let stack_ptr = addr_space.map_stack().unwrap();
    let trap_context = TrapContext::new(entry_point.to_usize(), stack_ptr.to_usize());
    let task_inner = TaskInner::new();
    let task = Arc::new(Task {
        tid: tid_alloc(),
        process: None,
        is_process: true,
        trap_context: unsafe { SyncUnsafeCell::new(trap_context) },
        timer: unsafe { SyncUnsafeCell::new(TaskTimeStat::new()) },
        waker: unsafe { SyncUnsafeCell::new(None) },
        state: unsafe { SpinNoIrqLock::new(TaskState::Waiting) },
        addr_space: SpinNoIrqLock::new(addr_space),
        inner: unsafe { UPSafeCell::new(task_inner) },
    });
    add_task(&task);
    spawn_user_task(task);
}
