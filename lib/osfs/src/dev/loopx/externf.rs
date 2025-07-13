use mutex::ShareMutex;

use crate::fd_table::FdTable;

#[crate_interface::def_interface]
pub trait KernelTableIf {
    fn table() -> ShareMutex<FdTable>;
}
