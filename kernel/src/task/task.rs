use crate::task::tid::{Tid, TidHandle, tid_alloc};

extern crate alloc;
use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use mutex::{ShareMutex, SpinNoIrqLock, new_share_mutex};

use core::cell::SyncUnsafeCell;
use core::task::Waker;

use time::TaskTimeStat;

use crate::trap::trap_context::TrapContext;
use crate::vm::addr_space::AddrSpace;

use super::tid::Pid;

/// State of Task
///
/// Running: When the task is running, in task_executor_unit loop
///
/// Zombie:  When the task exits and wait for the initproc to recycle it
///
/// Waiting: When the waiting syscall reaches, the task will be set and suspended
///
/// Sleeping: As Waiting. The difference is that its waiting time is longer
///
/// Interruptable: When the task is waiting for an long-time event such as I/O
///
/// UnInterruptable: As Interruptable. The difference is that it can not be interrupted by signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    Running,
    Zombie,
    Waiting,
    Sleeping,
    Interruptable,
    UnInterruptable,
}

#[derive(Debug)]
pub struct Task {
    tid: TidHandle,
    process: Option<Weak<Task>>,
    is_process: bool,

    trap_context: SyncUnsafeCell<TrapContext>,
    trap_context_spinlock: SpinNoIrqLock<TrapContext>,
    timer: SyncUnsafeCell<TaskTimeStat>,
    waker: SyncUnsafeCell<Option<Waker>>,
    state: SpinNoIrqLock<TaskState>,
    addr_space: ShareMutex<AddrSpace>,

    parent: ShareMutex<Option<Weak<Task>>>,
    children: ShareMutex<BTreeMap<Tid, Weak<Task>>>,

    exit_code: SpinNoIrqLock<i32>,
}

/// This Impl is mainly for getting and setting the property of Task
impl Task {
    pub fn new(entry: usize, sp: usize, addrspace: AddrSpace) -> Self {
        Task {
            tid: tid_alloc(),
            process: None,
            is_process: false,
            trap_context: SyncUnsafeCell::new(TrapContext::new(entry, sp)),
            trap_context_spinlock: SpinNoIrqLock::new(TrapContext::new(entry, sp)),
            timer: SyncUnsafeCell::new(TaskTimeStat::new()),
            waker: SyncUnsafeCell::new(None),
            state: SpinNoIrqLock::new(TaskState::Running),
            addr_space: Arc::new(SpinNoIrqLock::new(addrspace)),
            parent: new_share_mutex(None),
            children: new_share_mutex(BTreeMap::new()),
            exit_code: SpinNoIrqLock::new(0),
        }
    }

    // ========== This Part You Can Get the Member of Task ===========
    // Attention: Returns include reference and clone

    pub fn tid(&self) -> Tid {
        self.tid.0
    }

    /// Returns its process tid or its own tid if it's a process.
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

    pub fn get_state(&self) -> TaskState {
        self.state.lock().clone()
    }

    #[allow(clippy::mut_from_ref)]
    pub fn trap_context_mut(&self) -> &mut TrapContext {
        unsafe { &mut *self.trap_context.get() }
    }

    pub fn trap_context_spinlock_mut(&self) -> &SpinNoIrqLock<TrapContext> {
        &self.trap_context_spinlock
    }

    #[allow(clippy::mut_from_ref)]
    pub fn timer_mut(&self) -> &mut TaskTimeStat {
        unsafe { &mut *self.timer.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn waker_mut(&self) -> &mut Option<Waker> {
        unsafe { &mut *self.waker.get() }
    }

    pub fn addr_space_mut(&self) -> &SpinNoIrqLock<AddrSpace> {
        &self.addr_space
    }

    pub fn get_waker(&self) -> Waker {
        self.waker_mut().as_ref().unwrap().clone()
    }

    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.lock().clone()
    }

    // ========== This Part You Can Check the State of Task  ===========
    pub fn is_process(&self) -> bool {
        self.is_process
    }

    pub fn is_in_state(&self, state: TaskState) -> bool {
        self.get_state() == state
    }

    // ========== This Part You Can Set the Member of Task  ===========
    pub fn set_state(&self, state: TaskState) {
        *self.state.lock() = state;
    }

    pub fn set_parent(&mut self, parent: Arc<Task>) {
        *self.parent.lock() = Some(Arc::downgrade(&parent));
    }

    pub fn set_exit_code(&self, exit_code: i32) {
        *self.exit_code.lock() = exit_code;
    }

    pub fn set_waker(&self, waker: Waker) {
        *self.waker_mut() = Some(waker);
    }

    // ========== This Part You Can Change the Member of Task  ===========
    pub fn add_child(&mut self, child: Arc<Task>) {
        self.children
            .lock()
            .insert(child.tid(), Arc::downgrade(&child));
    }

    pub fn remove_child(&mut self, child: Arc<Task>) {
        self.children.lock().remove(&child.tid());
    }
}
