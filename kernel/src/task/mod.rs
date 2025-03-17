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
    let elf_data = get_app_data_by_name("hello_world").unwrap();
    log::debug!("try to load init data in app");
    Task::spawn_from_elf(elf_data);
}
