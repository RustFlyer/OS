use alloc::{collections::BTreeMap, string::String, sync::Arc};

use config::vfs::OpenFlags;
use mutex::ShareMutex;
use osfs::{
    dev::loopx::externf::KernelTableIf,
    fd_table::FdTable,
    proc::{KernelProcIf, fdinfo::info::ProcFdInfo},
};
use systype::{
    error::{SysError, SysResult},
    kinterface::KernelTaskOperations,
};
use vfs::{fanotify::kinterface::KernelFdTableOperations, file::File};

use super::manager::TASK_MANAGER;
use crate::{processor::current_task, trap::trap_handler::TRAP_STATS};

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> String {
        unsafe { current_task().elf().dentry().path() }
    }

    fn status() -> String {
        current_task().proc_status_read()
    }

    fn stat() -> String {
        current_task().proc_stat_read()
    }

    fn stat_from_tid(tid: usize) -> String {
        if let Some(task) = TASK_MANAGER.get_task(tid) {
            return task.proc_stat_read();
        }
        log::error!("no task {}", tid);
        return String::new();
    }

    fn maps() -> String {
        current_task().proc_maps_read()
    }

    fn maps_from_tid(tid: usize) -> String {
        if let Some(task) = TASK_MANAGER.get_task(tid) {
            return task.proc_maps_read();
        }
        log::error!("no task {}", tid);
        return String::new();
    }

    fn interrupts() -> BTreeMap<usize, usize> {
        TRAP_STATS.get_all()
    }

    fn fd(fd: usize) -> String {
        current_task().with_mut_fdtable(|table| table.get_file(fd).unwrap().dentry().path())
    }

    fn fdinfo_from_tid_and_fd(tid: usize, fd: usize) -> SysResult<ProcFdInfo> {
        let task = TASK_MANAGER.get_task(tid).ok_or(SysError::EINVAL)?;
        let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;

        let ret = ProcFdInfo {
            flags: file.flags().bits() as u32,
            pos: file.pos() as u64,
            minflt: 0,
            majflt: 0,
            nflock: 1,
        };

        Ok(ret)
    }
}

struct KernelTableIfImpl;

#[crate_interface::impl_interface]
impl KernelTableIf for KernelTableIfImpl {
    fn table() -> ShareMutex<FdTable> {
        let task = current_task();
        task.fdtable_mut()
    }
}

struct KernelFdTableOperationsImpl;

#[crate_interface::impl_interface]
impl KernelFdTableOperations for KernelFdTableOperationsImpl {
    fn add_file(file: Arc<dyn File>, flags: OpenFlags) -> SysResult<i32> {
        let task = current_task();
        task.fdtable_mut()
            .lock()
            .alloc(file, flags)
            .map(|fd| fd as i32)
    }
}

struct KernelTaskOperationsImpl;

#[crate_interface::impl_interface]
impl KernelTaskOperations for KernelTaskOperationsImpl {
    fn current_pid() -> i32 {
        current_task().pid() as i32
    }

    fn current_tid() -> i32 {
        current_task().tid() as i32
    }
}
