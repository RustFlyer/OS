use crate::task::tid::{Tid, TidHandle, tid_alloc};
use sig_member::*;
use signal::sig_info::*;

extern crate alloc;
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};
use driver::println;
use mm::vm::addr_space::AddrSpace;
use mutex::{ShareMutex, SpinNoIrqLock, new_share_mutex};

use core::cell::SyncUnsafeCell;
use core::task::Waker;

use time::TaskTimeStat;

use crate::trap::trap_context::TrapContext;

use super::{
    threadgroup::ThreadGroup,
    tid::{PGid, Pid},
};

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

    threadgroup: ShareMutex<ThreadGroup>,

    trap_context: SyncUnsafeCell<TrapContext>,
    timer: SyncUnsafeCell<TaskTimeStat>,
    waker: SyncUnsafeCell<Option<Waker>>,
    state: SpinNoIrqLock<TaskState>,
    addr_space: ShareMutex<AddrSpace>,

    parent: ShareMutex<Option<Weak<Task>>>,
    children: ShareMutex<BTreeMap<Tid, Weak<Task>>>,

    pgid: ShareMutex<PGid>,
    exit_code: SpinNoIrqLock<i32>,

    sig_mask: SyncUnsafeCell<SigSet>,
    sig_handlers: ShareMutex<SigHandlers>,
    sig_manager: SpinNoIrqLock<SigManager>,
    sig_stack: SyncUnsafeCell<Option<SignalStack>>,

    name: String,
}

/// This Impl is mainly for getting and setting the property of Task
impl Task {
    pub fn new(entry: usize, sp: usize, addrspace: AddrSpace, name: String) -> Self {
        let tid = tid_alloc();
        let pgid = tid.0;
        Task {
            tid,
            process: None,
            is_process: false,
            threadgroup: new_share_mutex(ThreadGroup::new()),
            trap_context: SyncUnsafeCell::new(TrapContext::new(entry, sp)),
            timer: SyncUnsafeCell::new(TaskTimeStat::new()),
            waker: SyncUnsafeCell::new(None),
            state: SpinNoIrqLock::new(TaskState::Running),
            addr_space: Arc::new(SpinNoIrqLock::new(addrspace)),
            parent: new_share_mutex(None),
            children: new_share_mutex(BTreeMap::new()),
            pgid: new_share_mutex(pgid),
            exit_code: SpinNoIrqLock::new(0),
            sig_manager: SpinNoIrqLock::new(SigManager::new()),
            sig_mask: SyncUnsafeCell::new(SigSet::empty()),
            sig_handlers: new_shared(SigHandlers::new()),
            sig_stack: SyncUnsafeCell::new(None),
            name,
        }
    }

    pub fn new_fork_clone(
        tid: TidHandle,
        process: Option<Weak<Task>>,
        is_process: bool,

        threadgroup: ShareMutex<ThreadGroup>,

        trap_context: SyncUnsafeCell<TrapContext>,
        timer: SyncUnsafeCell<TaskTimeStat>,
        waker: SyncUnsafeCell<Option<Waker>>,
        state: SpinNoIrqLock<TaskState>,
        addr_space: ShareMutex<AddrSpace>,

        parent: ShareMutex<Option<Weak<Task>>>,
        children: ShareMutex<BTreeMap<Tid, Weak<Task>>>,

        pgid: ShareMutex<PGid>,
        exit_code: SpinNoIrqLock<i32>,
        name: String,
    ) -> Self {
        Task {
            tid,
            process,
            is_process,
            threadgroup,
            trap_context,
            timer,
            waker,
            state,
            addr_space,
            parent,
            children,
            pgid,
            exit_code,
            name,
            sig_mask,
            sig_handlers,
            sig_manager,
            sig_stack,
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

    #[allow(clippy::mut_from_ref)]
    pub fn timer_mut(&self) -> &mut TaskTimeStat {
        unsafe { &mut *self.timer.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn waker_mut(&self) -> &mut Option<Waker> {
        unsafe { &mut *self.waker.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn sig_manager_mut(&self) -> &mut SigManager {
        unsafe { &mut self.sig_manager.lock() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn sig_handlers_mut(&self) -> &mut SigHandlers {
        unsafe { &mut self.sig_handlers.lock() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn sig_stack_mut(&self) -> &mut Option<SignalStack> {
        unsafe { &mut *self.sig_stack.get() }
    }

    pub fn addr_space_mut(&self) -> &ShareMutex<AddrSpace> {
        &self.addr_space
    }

    pub fn parent_mut(&self) -> &ShareMutex<Option<Weak<Task>>> {
        &self.parent
    }

    pub fn children_mut(&self) -> &ShareMutex<BTreeMap<Tid, Weak<Task>>> {
        &self.children
    }

    pub fn pgid_mut(&self) -> &ShareMutex<PGid> {
        &self.pgid
    }

    pub fn get_waker(&self) -> Waker {
        self.waker_mut().as_ref().unwrap().clone()
    }

    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.lock().clone()
    }

    pub fn get_pgid(&self) -> PGid {
        self.pgid.lock().clone()
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_sig_mask(&self) -> SigSet {
        self.sig_mask.lock().clone()
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

    pub fn set_pgid(&self, pgid: PGid) {
        *self.pgid_mut().lock() = pgid;
    }
    // ========== This Part You Can Change the Member of Task  ===========
    pub fn add_child(&self, child: Arc<Task>) {
        self.children
            .lock()
            .insert(child.tid(), Arc::downgrade(&child));
    }

    pub fn remove_child(&self, child: Arc<Task>) {
        self.children.lock().remove(&child.tid());
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        let str = format!("Task [{}] is drop", self.get_name());
        log::info!("{}", str);
        log::error!("{}", str);
        log::debug!("{}", str);
        log::trace!("{}", str);
    }
}
