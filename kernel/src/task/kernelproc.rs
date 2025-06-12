use osfs::proc::KernelProcIf;

use crate::processor::current_task;

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> alloc::string::String {
        unsafe { current_task().elf().dentry().path() }
    }

    fn status() -> alloc::string::String {
        current_task().proc_status_read()
    }
}
