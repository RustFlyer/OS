use alloc::string::String;
use mutex::ShareMutex;
use osfs::{dev::loopx::externf::KernelTableIf, fd_table::FdTable, proc::KernelProcIf};

use crate::processor::current_task;

use super::manager::TASK_MANAGER;

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> alloc::string::String {
        unsafe { current_task().elf().dentry().path() }
    }

    fn status() -> alloc::string::String {
        current_task().proc_status_read()
    }

    fn stat() -> alloc::string::String {
        current_task().proc_stat_read()
    }

    fn stat_from_tid(tid: usize) -> alloc::string::String {
        if let Some(task) = TASK_MANAGER.get_task(tid) {
            return task.proc_stat_read();
        }
        log::error!("no task {}", tid);
        return String::new();
    }
}

struct KernelTableIfImpl;

#[crate_interface::impl_interface]
impl KernelTableIf for KernelProcIfImpl {
    fn table() -> ShareMutex<FdTable> {
        let task = current_task();
        task.fdtable_mut()
    }
}
