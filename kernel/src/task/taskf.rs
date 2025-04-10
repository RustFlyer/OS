use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use config::vfs::AtFd;
use core::cell::SyncUnsafeCell;
use core::time::Duration;
use log::info;
use osfs::sys_root_dentry;
use sbi_rt::legacy::send_ipi;
use systype::SysResult;
use vfs::dentry::Dentry;
use vfs::file::File;
use vfs::path::Path;

use riscv::asm::sfence_vma_all;

use arch::riscv64::time::get_time_duration;
use config::process::CloneFlags;
use mutex::SpinNoIrqLock;
use mutex::new_share_mutex;
use osfs::fd_table::FdTable;
use time::TaskTimeStat;
use timer::{TIMER_MANAGER, Timer};

use super::future::{self};
use super::manager::TASK_MANAGER;
use super::process_manager::PROCESS_GROUP_MANAGER;
use super::task::*;
use super::threadgroup::ThreadGroup;
use super::tid::tid_alloc;
use crate::vm::addr_space::{AddrSpace, switch_to};

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

    pub fn init_stack(
        &self,
        stack_top: usize,
        argv: Vec<String>,
        envp: Vec<String>,
    ) -> (usize, usize, usize, usize) {
        self.addr_space_mut()
            .lock()
            .init_stack(stack_top, argv.len(), argv, envp)
    }

    pub fn execve(
        &self,
        elf_file: Arc<dyn File>,
        argv: Vec<String>,
        envp: Vec<String>,
        name: &str,
    ) {
        let mut addrspace = AddrSpace::build_user().unwrap();
        let entry_point = addrspace.load_elf(elf_file.clone()).unwrap();
        let stack = addrspace.map_stack().unwrap();
        addrspace.map_heap().unwrap();

        *self.addr_space_mut().lock() = addrspace;
        self.switch_addr_space();

        *self.args_mut() = argv.clone();
        let (sp, argc, argv, envp) = self.init_stack(stack.to_usize(), argv, envp);

        self.trap_context_mut()
            .init_user(sp, entry_point.to_usize(), argc, argv, envp);

        *self.elf_mut() = elf_file;

        *self.name_mut() = name.to_string();

        self.with_mut_fdtable(|table| table.close());
    }

    /// Spawns from Elf
    pub fn spawn_from_elf(elf_file: Arc<dyn File>, name: &str) {
        let mut addrspace = AddrSpace::build_user().unwrap();
        let entry_point = addrspace.load_elf(elf_file.clone()).unwrap();
        let stack = addrspace.map_stack().unwrap();
        addrspace.map_heap().unwrap();
        let task = Arc::new(Task::new(
            entry_point.to_usize(),
            stack.to_usize(),
            addrspace,
            elf_file,
            Vec::new(),
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
    /// - todo1: Thread Control
    /// - todo2: Sig Clone?
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

        let cwd = new_share_mutex(self.cwd_mut());

        let elf = SyncUnsafeCell::new((*self.elf_mut()).clone());
        let args = SyncUnsafeCell::new((*self.args_mut()).clone());

        let mut name = self.get_name() + "(fork)";

        if cloneflags.contains(CloneFlags::THREAD) {
            is_process = false;
            process = Some(Arc::downgrade(self));
            threadgroup = self.thread_group_mut().clone();
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
            name = name + "(thread)";
        } else {
            let cow_address_space = self.addr_space_mut().lock().clone_cow().unwrap();
            addr_space = Arc::new(SpinNoIrqLock::new(cow_address_space));
            sfence_vma_all();
        }

        let fd_table = if cloneflags.contains(CloneFlags::FILES) {
            self.fdtable_mut().clone()
        } else {
            new_share_mutex(self.fdtable_mut().lock().clone())
        };

        let name = SyncUnsafeCell::new(name);

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
            fd_table,
            cwd,
            elf,
            args,
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

    pub fn resolve_path(&self, dirfd: AtFd, pathname: String) -> SysResult<Arc<dyn Dentry>> {
        let p = if pathname.starts_with("/") {
            let path = Path::new(sys_root_dentry(), sys_root_dentry(), &pathname);
            path.walk()?
        } else {
            match dirfd {
                AtFd::FdCwd => {
                    let path = Path::new(sys_root_dentry(), self.cwd_mut(), &pathname);
                    path.walk()?
                }
                AtFd::Normal(fd) => {
                    let file = self.with_mut_fdtable(|table| table.get_file(fd))?;
                    Path::new(sys_root_dentry(), file.dentry(), &pathname).walk()?
                }
            }
        };

        Ok(p)
    }

    pub fn exit(&self) {
        self.set_state(TaskState::Zombie);
    }
}
