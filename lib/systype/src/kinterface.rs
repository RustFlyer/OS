use crate_interface::def_interface;

#[def_interface]
pub trait KernelTaskOperations {
    /// Returns the current task's PID.
    fn current_pid() -> i32;

    /// Returns the current task's TID.
    fn current_tid() -> i32;
}
