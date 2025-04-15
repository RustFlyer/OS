use crate::task::Task;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::sync::Weak;
use driver::println;
use mutex::SpinNoIrqLock;

use crate::task::tid::Tid;
use lazy_static::lazy_static;

lazy_static! {
    /// Task Manager
    ///
    /// Task Control of All the Tasks including Processes and Threads.
    pub static ref TASK_MANAGER: TaskManager = TaskManager::new();
}

pub struct TaskManager(SpinNoIrqLock<BTreeMap<Tid, Weak<Task>>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_task(&self, task: &Arc<Task>) {
        println!("[add_task] {}", task.tid());
        self.0.lock().insert(task.tid(), Arc::downgrade(task));
    }

    pub fn remove_task(&self, tid: Tid) {
        println!("[remove_task] {tid}");
        self.0.lock().remove(&tid);
    }

    pub fn get_task(&self, tid: Tid) -> Option<Arc<Task>> {
        if let Some(task) = self.0.lock().get(&tid) {
            task.upgrade()
        } else {
            None
        }
    }
}

pub fn add_task(task: &Arc<Task>) {
    TASK_MANAGER.add_task(task);
}
