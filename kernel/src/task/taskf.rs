use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use arch::time::get_time_ms;
use arch::time::get_time_us;
use core::cell::SyncUnsafeCell;
use core::sync::atomic::AtomicUsize;
use core::time::Duration;
use osfuture::suspend_now;

use arch::time::get_time_duration;
use config::process::CloneFlags;
use config::process::INIT_PROC_ID;
use config::vfs::AtFd;
use mutex::SpinNoIrqLock;
use mutex::new_share_mutex;
use osfs::sys_root_dentry;
use systype::SysResult;
use time::TaskTimeStat;
use timer::{TIMER_MANAGER, Timer};
use vfs::dentry::Dentry;
use vfs::file::File;
use vfs::path::Path;

use super::future::{self};
use super::manager::TASK_MANAGER;
use super::process_manager::PROCESS_GROUP_MANAGER;
use super::sig_members::SigHandlers;
use super::sig_members::SigManager;
use super::task::*;
use super::threadgroup::ThreadGroup;
use super::tid::TidAddress;
use super::tid::tid_alloc;

use crate::task::signal::sig_info::{Sig, SigDetails, SigInfo};
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
        suspend_now().await;
        let now = get_time_duration();
        if expire > now {
            expire - now
        } else {
            Duration::ZERO
        }
    }

    /// `execve()` executes the program with `elf_file`. `execve()` can extract
    /// elf data from `elf_file`. Then `execve()` builds a new user memory space
    /// and maps new stack and new heap. After setting addrspace, `evecve()`
    /// switches its addrspace(switch current pagetable in satp).
    ///
    /// Then this function initializes user stack, pushing argvs, envs and some
    /// relevant infos into stack. And it initializes trap context to get ready
    /// for switching from user space from kernel space.
    ///
    /// there is no new process; many attributes of the calling process remain
    /// unchanged. All that `execve()` does is arrange for an existing process
    /// (the calling process) to execute a new program.
    ///
    /// Attention: all threads other than the calling thread need to be destroyed
    /// during an `execve()`. Mutexes, condition variables, and other pthreads are
    /// not preserved.
    ///
    /// By default, file descriptors remain open across an `execve()`. File descriptors
    /// that are marked close-on-exec are closed.
    ///
    /// # Attributes Not Preserved
    /// - [ ] The dispositions of any `signals` that are `being caught` are reset to the default.
    /// - [ ] Any alternative `signal stack` is not preserved.
    /// - [x] `Memory mappings` are not preserved.
    /// - [ ] Attached System V `shared memory segments` are detached.
    /// - [x] POSIX `timers` are not preserved.
    /// - [ ] POSIX `shared memory regions` are unmapped.
    /// - [ ] open POSIX message queue `descriptors`.
    /// - [ ] Any open POSIX named `semaphores` are closed.
    /// - [ ] Any open `directory` streams are closed.
    /// - [ ] `Memorylocks` are not preserved.
    /// - [ ] `Exit handlers` are not preserved.
    /// - [x] The `floating-point environment` is reset to the default.
    pub fn execve(
        &self,
        elf_file: Arc<dyn File>,
        args: Vec<String>,
        envs: Vec<String>,
        name: String,
    ) -> SysResult<()> {
        let addrspace = AddrSpace::build_user()?;
        let (entry_point, auxv) = addrspace.load_elf(elf_file.clone())?;
        // log::debug!("[execve] load elf: over");
        let stack_top = addrspace.map_stack()?;
        addrspace.map_heap()?;

        // SAFETY: We should destroy other threads of this process before,
        // but multi-threading is not supported now, so this is safe.
        unsafe {
            self.set_addrspace(addrspace);
        }
        self.switch_addr_space();

        // Use current time as random seed
        let mut random = Vec::new();
        random.extend(get_time_us().to_le_bytes());
        random.extend(get_time_ms().to_le_bytes());
        let random: [u8; 16] = random.as_slice().try_into().unwrap();

        let addrspace = self.addr_space();
        let (sp, argc, argv, envp) =
            addrspace.init_stack(stack_top.to_usize(), args, envs, auxv, &random)?;

        self.trap_context_mut()
            .init_user(sp, entry_point.to_usize(), argc, argv, envp);

        *self.timer_mut() = TaskTimeStat::new();
        *self.elf_mut() = elf_file;
        *self.name_mut() = name;
        self.with_mut_fdtable(|table| table.close());

        Ok(())
    }

    /// Spawns from Elf
    pub fn spawn_from_elf(elf_file: Arc<dyn File>, name: &str) {
        let addrspace = AddrSpace::build_user().unwrap();
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
    pub fn switch_addr_space(&self) {
        unsafe {
            let addrspace = self.addr_space();
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

        let sig_mask = SyncUnsafeCell::new(self.get_sig_mask());
        let sig_handlers = new_share_mutex(SigHandlers::new());
        let sig_manager = SyncUnsafeCell::new(SigManager::new());
        let sig_stack = SyncUnsafeCell::new(*self.sig_stack_mut());
        let sig_cx_ptr = AtomicUsize::new(0);

        let addr_space = if cloneflags.contains(CloneFlags::VM) {
            name += "(thread)";
            self.addr_space()
        } else {
            let cow_address_space = self.addr_space().clone_cow().unwrap();
            Arc::new(cow_address_space)
        };

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
            SyncUnsafeCell::new(addr_space),
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

    /// Performs path resolution for a given path relative to a directory file
    /// descriptor of the current process.
    ///
    /// This function performs path resolution as `*at` syscalls do. It takes a file
    /// descriptor as the base directory and a path string, and returns the dentry
    /// corresponding to the path.
    ///
    /// This function returns the target dentry itself, even if the path is a
    /// symbolic link. The caller is responsible for resolving the symbolic link
    /// if it needs to do so.
    ///
    /// This function returns an invalid dentry if the target file does not exist.
    /// The caller should check the dentry's validity if it requires the target file
    /// to exist.
    ///
    /// If the path is absolute (starts with `/`), it is resolved from the root
    /// directory. If the path is relative, it is resolved from the directory
    /// specified by the file descriptor. In the latter case, the parameter `dirfd`
    /// specifies which directory to use as the base. If `dirfd` is `FdCwd`,
    /// the current working directory is used. If `dirfd` is `Normal(fd)`, the file
    /// associated with the file descriptor `fd` is used as the base directory.
    ///
    /// # Errors
    /// Returns an `EBADF` error if `dirfd` is invalid.
    ///
    /// See [`Path::walk`] for more details on what errors may be returned.
    pub fn walk_at(&self, dirfd: AtFd, path: String) -> SysResult<Arc<dyn Dentry>> {
        let base_dir = if path.starts_with("/") {
            sys_root_dentry()
        } else {
            match dirfd {
                AtFd::FdCwd => self.cwd_mut(),
                AtFd::Normal(fd) => self.with_mut_fdtable(|table| table.get_file(fd))?.dentry(),
            }
        };
        Path::new(base_dir, path).walk()
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
            // log::error!("{}", guard.len());
            assert!(guard.len() == 1);
        } else {
            assert!(guard.len() == 2);
            // NOTE: leader will be removed by parent calling `sys_wait4`
            log::error!("[exit] remove1 {}", self.tid());
            guard.remove(self);
            TASK_MANAGER.remove_task(self.tid());
        }

        log::info!("[Task::do_exit] exit the whole process");

        log::debug!("[Task::do_exit] reparent children to init");
        debug_assert_ne!(self.tid(), INIT_PROC_ID);

        //Question: Is mut safe here?
        let mut children = self.children_mut().lock().clone();
        if !children.is_empty() {
            let root = TASK_MANAGER.get_task(INIT_PROC_ID).unwrap();
            for child in children.values() {
                log::debug!(
                    "[Task::do_eixt] reparent child process pid {} to init",
                    child.pid()
                );
                if child.get_state() == TaskState::WaitForRecycle {
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
        }

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
            self.set_state(TaskState::WaitForRecycle);
        } else {
            self.process().set_state(TaskState::WaitForRecycle);
        }
        // When the task is not leader, which means its is not a process, it
        // will get dropped when hart leaves this task.
    }
}
