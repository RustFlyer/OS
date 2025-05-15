use osfs::proc::exe::file::KernelProcIf;

use crate::processor::current_task;

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> alloc::string::String {
        unsafe { current_task().elf().dentry().path() }
    }
}
