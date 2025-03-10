use crate::task::Task;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use mutex::SpinNoIrqLock;

use crate::task::tid::Tid;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = TaskManager::new();
}

pub struct TaskManager(SpinNoIrqLock<BTreeMap<Tid, Arc<Task>>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_task(&self, task: Arc<Task>) {
        self.0.lock().insert(task.tid(), task);
    }

    pub fn remove_task(&self, tid: Tid) {
        self.0.lock().remove(&tid);
    }

    pub fn get_task(&self, tid: Tid) -> Option<Arc<Task>> {
        if let Some(task) = self.0.lock().get(&tid) {
            Some(task.clone())
        } else {
            None
        }
    }
}

pub fn add_task(task: Arc<Task>) {
    TASK_MANAGER.add_task(task);
}
