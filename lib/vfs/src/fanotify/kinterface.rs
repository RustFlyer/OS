use alloc::sync::Arc;

use config::vfs::OpenFlags;
use crate_interface::def_interface;

use systype::error::SysResult;

use crate::file::File;

#[def_interface]
pub trait KernelFdTableOperations {
    /// Adds a [`File`] to the file descriptor table of the current task.
    fn add_file(file: Arc<dyn File>, flags: OpenFlags) -> SysResult<i32>;
}
