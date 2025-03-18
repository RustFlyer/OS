pub mod future;
pub mod manager;
pub mod task;
pub mod tid;

pub use future::yield_now;
pub use manager::TASK_MANAGER;
pub use task::{Task, TaskState};
pub use tid::{Tid, TidHandle, tid_alloc};

extern crate alloc;

use alloc::sync::Arc;

use crate::loader::get_app_data_by_name;

pub fn init() {
    let elf_data = get_app_data_by_name("add").unwrap();
    let elf_data2 = get_app_data_by_name("add1").unwrap();
    let elf_data3 = get_app_data_by_name("add2").unwrap();

    Task::spawn_from_elf(elf_data);
    Task::spawn_from_elf(elf_data2);
    Task::spawn_from_elf(elf_data3);

    // let elf_data2 = get_app_data_by_name("time_test").unwrap();
    // Task::spawn_from_elf(elf_data2);
}
