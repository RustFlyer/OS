use alloc::{collections::BTreeMap, string::String};

use mutex::ShareMutex;
use osfs::{dev::loopx::externf::KernelTableIf, fd_table::FdTable, proc::KernelProcIf};

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
}

struct KernelTableIfImpl;

#[crate_interface::impl_interface]
impl KernelTableIf for KernelTableIfImpl {
    fn table() -> ShareMutex<FdTable> {
        let task = current_task();
        task.fdtable_mut()
    }
}
