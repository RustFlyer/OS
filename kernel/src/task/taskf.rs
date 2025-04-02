extern crate alloc;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use mm::vm::addr_space::{AddrSpace, switch_to};
use riscv::asm::sfence_vma_all;
use time::TaskTimeStat;

use super::future::{self};
use super::manager::TASK_MANAGER;
use super::process_manager::{PROCESS_GROUP_MANAGER, ProcessGroupManager};
use super::threadgroup::ThreadGroup;
use super::tid::tid_alloc;
use super::{task::*, threadgroup};

use arch::riscv64::time::get_time_duration;
use config::process::CloneFlags;
use core::cell::SyncUnsafeCell;
use core::time::Duration;
use mutex::SpinNoIrqLock;
use mutex::new_share_mutex;
use timer::{TIMER_MANAGER, Timer};

impl Task {
    /// Switches Task to User
    pub fn enter_user_mode(&mut self) {
        self.timer_mut().switch_to_user();
    }

    /// Switches Task to Kernel
    pub fn enter_kernel_mode(&mut self) {
        self.timer_mut().switch_to_kernel();
    }

    /// Suspends the Task until it is waken or time out
    pub async fn suspend_timeout(&self, limit: Duration) -> Duration {
        let expire = limit;
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

    /// Spawns from Elf
    pub fn spawn_from_elf(elf_data: &'static [u8], name: &str) {
        let mut addrspace = AddrSpace::build_user().unwrap();
        let entry_point = addrspace.load_elf(elf_data).unwrap();
        let stack = addrspace.map_stack().unwrap();
        addrspace.map_heap().unwrap();
        let task = Arc::new(Task::new(
            entry_point.to_usize(),
            stack.to_usize(),
            addrspace,
            name.to_string(),
        ));

        PROCESS_GROUP_MANAGER.add_group(&task);
        TASK_MANAGER.add_task(&task);
        future::spawn_user_task(task);
    }

    /// Switches to the address space of the task.
    ///
    /// # Safety
    /// This function must be called before the current page table is dropped, or the kernel
    /// may lose its memory mappings.
    pub fn switch_addr_space(&self) {
        unsafe {
            switch_to(&self.addr_space_mut().lock());
        }
    }

    /// fork a application
    ///
    /// - todo1: Memory Copy / Cow
    /// - todo2: Control of relevant Thread Group
    pub fn fork(self: &Arc<Self>, cloneflags: CloneFlags) -> Arc<Self> {
        let tid = tid_alloc();
        let trap_context = SyncUnsafeCell::new(*self.trap_context_mut());
        let state = SpinNoIrqLock::new(self.get_state());

        let process;
        let is_process;
        let threadgroup;

        let parent;
        let children;

        let pgid;

        if cloneflags.contains(CloneFlags::THREAD) {
            is_process = false;
            process = Some(Arc::downgrade(self));
            threadgroup = new_share_mutex(ThreadGroup::new());
            parent = self.parent_mut().clone();
            children = self.children_mut().clone();
            pgid = self.pgid_mut().clone();
        } else {
            is_process = true;
            process = None;
            threadgroup = new_share_mutex(ThreadGroup::new());
            parent = new_share_mutex(Some(Arc::downgrade(self)));
            children = new_share_mutex(BTreeMap::new());

            pgid = new_share_mutex(self.get_pgid());
        }

        let addr_space;
        if cloneflags.contains(CloneFlags::VM) {
            addr_space = (*self.addr_space_mut()).clone();
        } else {
            let cow_address_space = self.addr_space_mut().lock().clone_cow().unwrap();
            addr_space = Arc::new(SpinNoIrqLock::new(cow_address_space));
            unsafe {
                sfence_vma_all();
            }
        }

        let name = self.get_name() + "(fork)";

        let new = Arc::new(Self::new_fork_clone(
            tid,
            process,
            is_process,
            threadgroup,
            trap_context,
            SyncUnsafeCell::new(TaskTimeStat::new()),
            SyncUnsafeCell::new(None),
            state,
            addr_space,
            parent,
            children,
            pgid,
            SpinNoIrqLock::new(0),
            name,
        ));

        if !cloneflags.contains(CloneFlags::THREAD) {
            self.add_child(new.clone());
        }

        if new.is_process() {
            PROCESS_GROUP_MANAGER.add_process(new.get_pgid(), &new);
        }

        TASK_MANAGER.add_task(&new);
        new
    }

    pub fn exit(&self) {
        self.set_state(TaskState::Zombie);
    }
}
