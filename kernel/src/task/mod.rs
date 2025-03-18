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
    let hello_world = get_app_data_by_name("hello_world").unwrap();
    let time_test = get_app_data_by_name("time_test").unwrap();
    let add = get_app_data_by_name("add").unwrap();
    let add1 = get_app_data_by_name("add1").unwrap();
    let add2 = get_app_data_by_name("add2").unwrap();

    Task::spawn_from_elf(hello_world);
    Task::spawn_from_elf(time_test);
    Task::spawn_from_elf(add);
    Task::spawn_from_elf(add1);
    Task::spawn_from_elf(add2);

    // let elf_data2 = get_app_data_by_name("time_test").unwrap();
    // Task::spawn_from_elf(elf_data2);
}
