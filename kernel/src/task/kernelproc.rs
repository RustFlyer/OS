use osfs::proc::KernelProcIf;

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
        TASK_MANAGER.get_task(tid).unwrap().proc_stat_read()
    }
}
