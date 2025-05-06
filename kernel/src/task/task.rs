use crate::task::sig_members::*;
use crate::task::signal::sig_info::*;
use crate::task::tid::{Tid, TidHandle, tid_alloc};

extern crate alloc;
use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
};
use core::cell::SyncUnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::Waker;
use mm::address::VirtAddr;
use mutex::{ShareMutex, SpinNoIrqLock, new_share_mutex};
use shm::id::ShmStat;
use time::itime::ITimer;
use vfs::{dentry::Dentry, file::File};

use osfs::{fd_table::FdTable, sys_root_dentry};
use time::TaskTimeStat;

use super::{
    threadgroup::ThreadGroup,
    tid::{PGid, Pid, TidAddress},
};
use crate::{trap::trap_context::TrapContext, vm::addr_space::AddrSpace};

/// State of Task
///
/// - Running: When the task is running, in task_executor_unit loop
/// - Zombie:  When the task exits and wait for the initproc to recycle it
/// - Waiting: When the waiting syscall reaches, the task will be set and suspended
/// - Sleeping: As Waiting. The difference is that its waiting time is longer
/// - Interruptable: When the task is waiting for an long-time event such as I/O
/// - UnInterruptable: As Interruptable. The difference is that it can not be interrupted by signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    Running,
    Zombie,
    WaitForRecycle,
    Sleeping,
    Interruptable,
    UnInterruptable,
}

pub struct Task {
    // Three members below are decided as birth and
    // can not be changed during the lifetime.

    // tid is the id of this thread. If is_process is
    // true, tid is also pid.
    tid: TidHandle,
    process: Option<Weak<Task>>,
    is_process: bool,

    // thread group takes control of tasks as threads
    // (include the task itself)
    threadgroup: ShareMutex<ThreadGroup>,

    // trap_context record context when the task switch
    // between kernel space and user space.
    trap_context: SyncUnsafeCell<TrapContext>,

    // timer records task time stat, such as kernel time
    // and user time. Further, it can affect task schedule
    // by its the rate cpu-time in all-time.
    timer: SyncUnsafeCell<TaskTimeStat>,

    // waker can wake up the user future that task belongs to
    // and affect task schedule.
    waker: SyncUnsafeCell<Option<Waker>>,

    // state refers to task state and control the direction of
    // a task.
    state: SpinNoIrqLock<TaskState>,

    // addr_space is task memory space, which is mapping
    // and organizing virtual address of a task.
    addr_space: SyncUnsafeCell<Arc<AddrSpace>>,

    // parent is task spawner. It spawn the task by fork or
    // clone and then parent is set as it.
    parent: ShareMutex<Option<Weak<Task>>>,

    // children controls all the task spawned by this task.
    // Attention: the pointer to children task is Arc. It's
    // because parent task should recycle children and free
    // them in wait4 at last.
    children: ShareMutex<BTreeMap<Tid, Arc<Task>>>,

    // pgid is the id of a process group if existed.
    pgid: ShareMutex<PGid>,

    // when task exits, it will set exit_code and wait for
    // parent task to clean it and receive exit_code.
    exit_code: SpinNoIrqLock<i32>,

    // sigmask is signal mask of task. When it is set
    // signal check will ignore its relevant signals.
    sig_mask: SyncUnsafeCell<SigSet>,

    // sig_handlers can handle signals by pre-set functions
    // when sig_check discovers new unmasked signals, it will
    // call relevant sig_handler functions and handles signals.
    // If sig_handlers are unset, signals will be handled by
    // default handlers in kernel.
    sig_handlers: ShareMutex<SigHandlers>,

    // sig_manager organizes signals received by task.
    sig_manager: SyncUnsafeCell<SigManager>,

    // sig_stack is the stack set specifically for signal-handlers.
    // It stores signal handler context and relevant signal infos.
    sig_stack: SyncUnsafeCell<Option<SignalStack>>,

    // sig_cx_ptr is ptr to sig_stack.
    sig_cx_ptr: AtomicUsize,

    // tid_address is the pointer to thread ID.
    tid_address: SyncUnsafeCell<TidAddress>,

    // fd_table stores open files fd and kernel can
    // find file by its fd and write or read it.
    fd_table: ShareMutex<FdTable>,

    // cwd is current working dentry. When AtFd::FdCwd is set,
    // task should use relative path with cwd.
    cwd: ShareMutex<Arc<dyn Dentry>>,

    // elf refers to the elf-file in disk and the task is running
    // with loading elf-file datas.
    elf: SyncUnsafeCell<Arc<dyn File>>,

    /// Map of start address of shared memory areas to their keys in the shared
    /// memory manager.
    shm_stats: ShareMutex<BTreeMap<VirtAddr, usize>>,

    is_syscall: AtomicBool,

    is_yield: AtomicBool,

    itimers: ShareMutex<[ITimer; 3]>,

    // name, used for debug
    name: SyncUnsafeCell<String>,
}

/// This Impl is mainly for getting and setting the property of Task
impl Task {
    pub fn new(
        entry: usize,
        sp: usize,
        addr_space: AddrSpace,
        elf_file: Arc<dyn File>,
        name: String,
    ) -> Self {
        let tid = tid_alloc();
        let pgid = tid.0;
        let task = Task {
            tid,
            process: None,
            is_process: false,
            threadgroup: new_share_mutex(ThreadGroup::new()),
            trap_context: SyncUnsafeCell::new(TrapContext::new(entry, sp)),
            timer: SyncUnsafeCell::new(TaskTimeStat::new()),
            waker: SyncUnsafeCell::new(None),
            state: SpinNoIrqLock::new(TaskState::Running),
            addr_space: SyncUnsafeCell::new(Arc::new(addr_space)),
            parent: new_share_mutex(None),
            children: new_share_mutex(BTreeMap::new()),
            pgid: new_share_mutex(pgid),
            exit_code: SpinNoIrqLock::new(0),
            sig_manager: SyncUnsafeCell::new(SigManager::new()),
            sig_mask: SyncUnsafeCell::new(SigSet::empty()),
            sig_handlers: new_share_mutex(SigHandlers::new()),
            sig_stack: SyncUnsafeCell::new(None),
            sig_cx_ptr: AtomicUsize::new(0),

            tid_address: SyncUnsafeCell::new(TidAddress::new()),
            fd_table: new_share_mutex(FdTable::new()),
            cwd: new_share_mutex(sys_root_dentry()),
            elf: SyncUnsafeCell::new(elf_file),
            shm_stats: new_share_mutex(BTreeMap::new()),
            is_syscall: AtomicBool::new(false),
            is_yield: AtomicBool::new(false),
            itimers: new_share_mutex([ITimer::default(); 3]),
            name: SyncUnsafeCell::new(name),
        };
        task
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
        addr_space: SyncUnsafeCell<Arc<AddrSpace>>,

        parent: ShareMutex<Option<Weak<Task>>>,
        children: ShareMutex<BTreeMap<Tid, Arc<Task>>>,

        pgid: ShareMutex<PGid>,
        exit_code: SpinNoIrqLock<i32>,

        sig_mask: SyncUnsafeCell<SigSet>,
        sig_handlers: ShareMutex<SigHandlers>,
        sig_manager: SyncUnsafeCell<SigManager>,
        sig_stack: SyncUnsafeCell<Option<SignalStack>>,
        sig_cx_ptr: AtomicUsize,

        tid_address: SyncUnsafeCell<TidAddress>,
        fd_table: ShareMutex<FdTable>,
        cwd: ShareMutex<Arc<dyn Dentry>>,
        elf: SyncUnsafeCell<Arc<dyn File>>,
        shm_stats: ShareMutex<BTreeMap<VirtAddr, usize>>,

        itimers: ShareMutex<[ITimer; 3]>,

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

            sig_mask,
            sig_handlers,
            sig_manager,
            sig_stack,
            sig_cx_ptr,

            tid_address,
            fd_table,
            cwd,
            elf,
            shm_stats,
            is_syscall: AtomicBool::new(false),
            is_yield: AtomicBool::new(false),
            itimers,
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
    pub fn sig_manager_mut(&self) -> &mut SigManager {
        unsafe { &mut *self.sig_manager.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn sig_handlers_mut(&self) -> &ShareMutex<SigHandlers> {
        &self.sig_handlers
    }

    #[allow(clippy::mut_from_ref)]
    pub fn sig_stack_mut(&self) -> &mut Option<SignalStack> {
        unsafe { &mut *self.sig_stack.get() }
    }

    pub fn addr_space(&self) -> Arc<AddrSpace> {
        unsafe { Arc::clone(&*self.addr_space.get()) }
    }

    pub fn raw_space_ptr(&self) -> usize {
        Arc::as_ptr(&self.addr_space()) as usize
    }

    pub fn elf_mut(&self) -> &mut Arc<dyn File> {
        unsafe { &mut *self.elf.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn name_mut(&self) -> &mut String {
        unsafe { &mut *self.name.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn tid_address_mut(&self) -> &mut TidAddress {
        unsafe { &mut *self.tid_address.get() }
    }

    pub fn parent_mut(&self) -> &ShareMutex<Option<Weak<Task>>> {
        &self.parent
    }

    pub fn children_mut(&self) -> &ShareMutex<BTreeMap<Tid, Arc<Task>>> {
        &self.children
    }

    pub fn pgid_mut(&self) -> &ShareMutex<PGid> {
        &self.pgid
    }

    pub fn sig_mask_mut(&self) -> &mut SigSet {
        unsafe { &mut *self.sig_mask.get() }
    }

    pub fn with_thread_group<T>(&self, f: impl FnOnce(&mut ThreadGroup) -> T) -> T {
        f(&mut self.threadgroup.lock())
    }

    pub fn with_mut_fdtable<T>(&self, f: impl FnOnce(&mut FdTable) -> T) -> T {
        f(&mut self.fd_table.lock())
    }

    pub fn with_mut_sig_manager<T>(&self, f: impl FnOnce(&mut SigManager) -> T) -> T {
        f(&mut self.sig_manager_mut())
    }

    pub fn with_mut_sig_handler<T>(&self, f: impl FnOnce(&mut SigHandlers) -> T) -> T {
        f(&mut self.sig_handlers_mut().lock())
    }

    pub fn with_mut_itimers<T>(&self, f: impl FnOnce(&mut [ITimer; 3]) -> T) -> T {
        f(&mut self.itimers.lock())
    }

    pub fn with_mut_shm_stats<T>(&self, f: impl FnOnce(&mut BTreeMap<VirtAddr, usize>) -> T) -> T {
        f(&mut self.shm_stats.lock())
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

    pub fn cwd(&self) -> ShareMutex<Arc<dyn Dentry>> {
        self.cwd.clone()
    }

    pub fn get_sig_mask(&self) -> SigSet {
        unsafe { *self.sig_mask.get() }
    }

    pub fn get_sig_cx_ptr(&self) -> usize {
        self.sig_cx_ptr.load(Ordering::Relaxed)
    }

    pub fn is_syscall(&self) -> bool {
        self.is_syscall.load(Ordering::Relaxed)
    }

    pub fn is_yield(&self) -> bool {
        self.is_yield.load(Ordering::Relaxed)
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

    pub fn set_sig_cx_ptr(&self, new_ptr: usize) {
        self.sig_cx_ptr.store(new_ptr, Ordering::Relaxed);
    }

    pub fn set_is_syscall(&self, is_syscall: bool) {
        self.is_syscall.store(is_syscall, Ordering::Relaxed);
    }

    pub fn set_is_yield(&self, is_yield: bool) {
        self.is_yield.store(is_yield, Ordering::Relaxed);
    }

    pub fn set_cwd(&self, dentry: Arc<dyn Dentry>) {
        *self.cwd.lock() = dentry;
    }

    /// Set the address space of the task
    ///
    /// # Safety
    /// The caller must ensure that no other hart is accessing the address space
    pub unsafe fn set_addrspace(&self, addrspace: AddrSpace) {
        unsafe {
            *self.addr_space.get() = Arc::new(addrspace);
        }
    }
    // ========== This Part You Can Change the Member of Task  ===========
    pub fn add_child(&self, child: Arc<Task>) {
        log::debug!("addchild: tid {} -> tid {} ", child.tid(), self.tid());
        self.children.lock().insert(child.tid(), child);
    }

    pub fn remove_child(&self, child: Arc<Task>) {
        log::debug!("child: tid [{}] will be removed", child.get_name());
        self.children.lock().remove(&child.tid());
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        let str = format!("Task [{}] is drop", self.get_name());

        let lock = self.parent_mut().lock();
        log::trace!(
            "Task [{}] parent [{}]",
            self.get_name(),
            lock.clone().unwrap().upgrade().unwrap().get_name()
        );

        self.children
            .lock()
            .values()
            .for_each(|c| log::debug!("children: tid [{}] name [{}]", c.tid(), c.get_name()));

        log::debug!("{}", str);
    }
}
