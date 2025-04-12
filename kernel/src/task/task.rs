use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::cell::SyncUnsafeCell;
use core::task::Waker;
use vfs::{dentry::Dentry, file::File};

use mutex::{ShareMutex, SpinNoIrqLock, new_share_mutex};
use osfs::{fd_table::FdTable, sys_root_dentry};
use time::TaskTimeStat;

use super::{
    threadgroup::ThreadGroup,
    tid::{PGid, Pid},
};
use crate::{
    task::tid::{Tid, TidHandle, tid_alloc},
    trap::trap_context::TrapContext,
    vm::addr_space::AddrSpace,
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

    fd_table: ShareMutex<FdTable>,
    cwd: ShareMutex<Arc<dyn Dentry>>,

    elf: SyncUnsafeCell<Arc<dyn File>>,

    name: SyncUnsafeCell<String>,
}

/// This Impl is mainly for getting and setting the property of Task
impl Task {
    pub fn new(
        entry: usize,
        sp: usize,
        addrspace: AddrSpace,
        elf_file: Arc<dyn File>,
        name: String,
    ) -> Self {
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
            fd_table: new_share_mutex(FdTable::new()),
            cwd: new_share_mutex(sys_root_dentry()),
            elf: SyncUnsafeCell::new(elf_file),
            name: SyncUnsafeCell::new(name),
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

        fd_table: ShareMutex<FdTable>,
        cwd: ShareMutex<Arc<dyn Dentry>>,

        elf: SyncUnsafeCell<Arc<dyn File>>,

        name: SyncUnsafeCell<String>,
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
            fd_table,
            cwd,
            elf,
            name,
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
            self.clone()
        } else {
            self.process.as_ref().cloned().unwrap().upgrade().unwrap()
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
    pub fn elf_mut(&self) -> &mut Arc<dyn File> {
        unsafe { &mut *self.elf.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn name_mut(&self) -> &mut String {
        unsafe { &mut *self.name.get() }
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

    pub fn with_mut_fdtable<T>(&self, f: impl FnOnce(&mut FdTable) -> T) -> T {
        f(&mut self.fd_table.lock())
    }

    pub fn cwd_mut(&self) -> Arc<dyn Dentry> {
        self.cwd.lock().clone()
    }

    pub fn fdtable_mut(&self) -> ShareMutex<FdTable> {
        self.fd_table.clone()
    }

    pub fn thread_group_mut(&self) -> ShareMutex<ThreadGroup> {
        self.threadgroup.clone()
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
        self.name_mut().clone()
    }

    pub fn ppid(&self) -> Pid {
        let parent = self.parent.lock().clone();
        parent
            .expect("Call ppid Without parent")
            .upgrade()
            .unwrap()
            .get_pgid()
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

    pub fn set_cwd(&self, dentry: Arc<dyn Dentry>) {
        *self.cwd.lock() = dentry;
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
        // log::info!("{}", str);
        // log::error!("{}", str);
        // log::debug!("{}", str);
        log::trace!("{}", str);
    }
}
