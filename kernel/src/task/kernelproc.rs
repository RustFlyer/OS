use osfs::proc::exe::file::KernelProcIf;

use crate::processor::current_task;

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> alloc::string::String {
        current_task().elf_mut().dentry().path()
    }
}
