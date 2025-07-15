use alloc::{format, string::String};

use crate_interface::call_interface;

pub mod dentry;
pub mod file;
pub mod inode;

/// Generate the interrupts output string
pub fn serialize_interrupts() -> String {
    let interrupts = call_interface!(super::KernelProcIf::interrupts());
    let mut result = String::new();

    for (irq_num, count) in interrupts.iter() {
        result += &format!("{}: {}\n", irq_num, count);
    }

    result
}
