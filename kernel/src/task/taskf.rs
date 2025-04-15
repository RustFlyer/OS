use super::future::{self};
use super::manager::TASK_MANAGER;
use super::process_manager::PROCESS_GROUP_MANAGER;
use super::sig_members::SigManager;
use super::task::*;
use super::threadgroup::ThreadGroup;
use super::tid::TidAddress;
use super::tid::tid_alloc;

use crate::task::signal::sig_info::{Sig, SigDetails, SigInfo};
use crate::vm::addr_space::{AddrSpace, switch_to};
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;

use arch::riscv64::time::get_time_duration;
use config::inode::InodeType;
use config::process::{CloneFlags, INIT_PROC_ID};
use config::vfs::AtFd;
use config::vfs::OpenFlags;

use core::cell::SyncUnsafeCell;
use core::sync::atomic::AtomicUsize;
use core::time::Duration;
use mutex::SpinNoIrqLock;
use mutex::new_share_mutex;
use mutex::optimistic_mutex::new_optimistic_mutex;

use osfs::sys_root_dentry;
use riscv::asm::sfence_vma_all;
use systype::SysResult;
use time::TaskTimeStat;
use timer::{TIMER_MANAGER, Timer};
use vfs::dentry::Dentry;
use vfs::file::File;
use vfs::path::Path;

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

    pub async fn execve(
        &self,
        elf_file: Arc<dyn File>,
        args: Vec<String>,
        envs: Vec<String>,
        name: String,
    ) -> SysResult<()> {
        let mut addrspace = AddrSpace::build_user()?;
        let (entry_point, auxv) = addrspace.load_elf(elf_file.clone())?;
        let stack_top = addrspace.map_stack()?;
        addrspace.map_heap()?;

        self.set_addrspace(addrspace).await;
        self.switch_addr_space().await;

        let mut addrspace = self.addr_space_mut().lock().await;
        let (sp, argc, argv, envp) =
            addrspace.init_stack(stack_top.to_usize(), args, envs, auxv)?;

        self.trap_context_mut()
            .init_user(sp, entry_point.to_usize(), argc, argv, envp);

        *self.elf_mut() = elf_file;
        *self.name_mut() = name;
        self.with_mut_fdtable(|table| table.close());

        Ok(())
    }

    /// Spawns from Elf
    pub fn spawn_from_elf(elf_file: Arc<dyn File>, name: &str) {
        let mut addrspace = AddrSpace::build_user().unwrap();
        let (entry_point, _) = addrspace.load_elf(elf_file.clone()).unwrap();
        log::debug!("[spawn_from_elf] entry: {:#x}", entry_point.to_usize());
        let stack = addrspace.map_stack().unwrap();
        addrspace.map_heap().unwrap();
        let task = Arc::new(Task::new(
            entry_point.to_usize(),
            stack.to_usize(),
            addrspace,
            elf_file,
            name.to_string(),
        ));
        task.with_thread_group(|tg| tg.push(task.clone()));
        PROCESS_GROUP_MANAGER.add_group(&task);
        TASK_MANAGER.add_task(&task);
        future::spawn_user_task(task);
    }

    /// Switches to the address space of the task.
    ///
    /// # Safety
    /// This function must be called before the current page table is dropped, or the kernel
    /// may lose its memory mappings.
    pub async fn switch_addr_space(&self) {
        unsafe {
            let addrspace = self.addr_space_mut().lock().await;
            switch_to(&addrspace);
        }
    }

    /// fork a application
    ///
    /// - todo1: Thread Control
    /// - todo2: Sig Clone?
    pub async fn fork(self: &Arc<Self>, cloneflags: CloneFlags) -> Arc<Self> {
        let tid = tid_alloc();
        let trap_context = SyncUnsafeCell::new(*self.trap_context_mut());
        let state = SpinNoIrqLock::new(self.get_state());

        let process;
        let is_process;
        let threadgroup;

        let parent;
        let children;

        let pgid;

        let sig_mask;
        let sig_handlers;
        let sig_manager;
        let sig_stack;
        let sig_cx_ptr;
        let cwd = new_share_mutex(self.cwd_mut());

        let elf = SyncUnsafeCell::new((*self.elf_mut()).clone());

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

        sig_mask = SyncUnsafeCell::new(self.get_sig_mask());
        sig_handlers = (*self.sig_handlers_mut()).clone();
        sig_manager = SyncUnsafeCell::new(SigManager::new());
        sig_stack = SyncUnsafeCell::new(self.sig_stack_mut().clone());
        sig_cx_ptr = AtomicUsize::new(0);

        let addr_space;
        if cloneflags.contains(CloneFlags::VM) {
            addr_space = (*self.addr_space_mut()).clone();
            name += "(thread)";
        } else {
            let cow_address_space = self.addr_space_mut().lock().await.clone_cow().unwrap();
            addr_space = new_optimistic_mutex(cow_address_space);
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
            sig_mask,
            sig_handlers,
            sig_manager,
            sig_stack,
            sig_cx_ptr,
            SyncUnsafeCell::new(TidAddress::new()),
            fd_table,
            cwd,
            elf,
            name,
        ));

        new.with_thread_group(|tg| tg.push(new.clone()));

        if !cloneflags.contains(CloneFlags::THREAD) {
            self.add_child(new.clone());
        }

        if new.is_process() {
            PROCESS_GROUP_MANAGER.add_process(new.get_pgid(), &new);
        }

        TASK_MANAGER.add_task(&new);
        new
    }

    pub fn wake(&self) {
        let waker = self.waker_mut();
        waker.as_ref().unwrap().wake_by_ref();
    }

    pub fn resolve_path(
        &self,
        dirfd: AtFd,
        pathname: String,
        flags: OpenFlags,
    ) -> SysResult<Arc<dyn Dentry>> {
        let dentry = if pathname.starts_with("/") {
            Path::new(sys_root_dentry(), pathname).walk()?
        } else {
            match dirfd {
                AtFd::FdCwd => Path::new(self.cwd_mut(), pathname).walk()?,
                AtFd::Normal(fd) => {
                    let file = self.with_mut_fdtable(|table| table.get_file(fd))?;
                    Path::new(file.dentry(), pathname).walk()?
                }
            }
        };

        if flags.contains(OpenFlags::O_NOFOLLOW) || dentry.is_negative() {
            Ok(dentry)
        } else if dentry.inode().unwrap().inotype() == InodeType::SymLink {
            Path::resolve_symlink_through(dentry)
        } else {
            Ok(dentry)
        }
    }

    pub fn exit(self: &Arc<Self>) {
        log::info!("thread {} do exit", self.tid());
        assert_ne!(
            self.tid(),
            INIT_PROC_ID,
            "initproc die!!!, sepc {:#x}",
            self.trap_context_mut().sepc
        );

        let tg_lock = self.thread_group_mut();
        let mut guard = tg_lock.lock();

        if (!(self.process().get_state() == TaskState::Zombie))
            || (self.is_process() && guard.len() > 1)
            || (!self.is_process() && guard.len() > 2)
        {
            if !self.is_process() {
                // NOTE: process will be removed by parent calling `sys_wait4`
                guard.remove(self);
                TASK_MANAGER.remove_task(self.tid());
            }
            return;
        }

        if self.is_process() {
            log::error!("{}", guard.len());
            assert!(guard.len() == 1);
        } else {
            assert!(guard.len() == 2);
            // NOTE: leader will be removed by parent calling `sys_wait4`
            guard.remove(self);
            TASK_MANAGER.remove_task(self.tid());
        }

        log::info!("[Task::do_exit] exit the whole process");

        log::debug!("[Task::do_exit] reparent children to init");
        debug_assert_ne!(self.tid(), INIT_PROC_ID);

        //Question: Is mut safe here?
        let mut children = self.children_mut().lock().clone();
        if children.is_empty() {
            // return;
        }
        let root = TASK_MANAGER.get_task(INIT_PROC_ID).unwrap();
        for c in children.values() {
            let child = c.upgrade().unwrap();
            log::debug!(
                "[Task::do_eixt] reparent child process pid {} to init",
                child.pid()
            );
            if child.get_state() == TaskState::Zombie {
                // NOTE: self has not called wait to clear zombie children, we need to notify
                // init to clear these zombie children.
                root.receive_siginfo(SigInfo {
                    sig: Sig::SIGCHLD,
                    code: SigInfo::CLD_EXITED,
                    details: SigDetails::None,
                })
            }
            // Question: Why Deref doesn't work here
            *child.parent_mut().lock() = Some(Arc::downgrade(&root));
        }
        root.children_mut().lock().extend(children.clone());
        children.clear();

        if let Some(parent) = self.parent_mut().lock().as_ref() {
            if let Some(parent) = parent.upgrade() {
                parent.receive_siginfo(SigInfo {
                    sig: Sig::SIGCHLD,
                    code: SigInfo::CLD_EXITED,
                    details: SigDetails::None,
                })
            } else {
                log::error!("no arc parent");
            }
        }

        // TODO: Upon _exit(2), all attached shared memory segments are detached from the
        // process.
        // self.with_shm_ids(|ids| {
        //     for (_, shm_id) in ids.iter() {
        //         SHARED_MEMORY_MANAGER.detach(*shm_id, self.pid());
        //     }
        // });

        // TODO: drop most resources here instead of wait4 function parent
        // called
        // self.with_mut_fd_table(|table| table.clear());

        if self.is_process() {
            self.set_state(TaskState::Zombie);
        } else {
            self.process().set_state(TaskState::Zombie);
        }
        // When the task is not leader, which means its is not a process, it
        // will get dropped when hart leaves this task.
    }
}
