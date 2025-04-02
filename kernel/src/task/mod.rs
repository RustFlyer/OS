pub mod future;
pub mod manager;
pub mod process_manager;
pub mod task;
pub mod taskf;
pub mod threadgroup;
pub mod tid;
pub mod signal;
pub mod sig_members;

pub use future::yield_now;

#[allow(unused)]
pub use manager::TASK_MANAGER;
pub use task::{Task, TaskState};

extern crate alloc;

use crate::loader::get_app_data_by_name;

pub fn init() {
    let init_proc = get_app_data_by_name("init_proc").unwrap();
    let hello_world = get_app_data_by_name("hello_world").unwrap();
    let time_test = get_app_data_by_name("time_test").unwrap();
    let add = get_app_data_by_name("add").unwrap();
    let add1 = get_app_data_by_name("add1").unwrap();
    let add2 = get_app_data_by_name("add2").unwrap();

    Task::spawn_from_elf(init_proc, "init_proc");
    Task::spawn_from_elf(hello_world, "hello_world");
    Task::spawn_from_elf(time_test, "time_test");
    Task::spawn_from_elf(add, "add");
    Task::spawn_from_elf(add1, "add1");
    Task::spawn_from_elf(add2, "add2");

    // let elf_data2 = get_app_data_by_name("time_test").unwrap();
    // Task::spawn_from_elf(elf_data2);
}
