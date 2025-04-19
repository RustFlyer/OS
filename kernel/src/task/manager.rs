use crate::task::Task;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::sync::Weak;
use mutex::SpinNoIrqLock;

use crate::task::tid::Tid;
use lazy_static::lazy_static;

lazy_static! {
/// Task Manager
///
/// Task Manager takes control of all the user tasks in the kernel, using a BTreeMap
/// struct as the tid is the key and the weak point to task is the value.
/// Task Manager is a global struct which is shared with all harts. Therefore, it's
/// guarded by a spin lock that disables the interrupt.
///
/// When a task is spawned, it should be added into the Task Manager. Also, the task should
/// be removed when it is terminated.
///
/// Task Manager should not disturb the life of a task so it does not ensure whether the task
/// is alive or not. When the task is terminated, `get_task` will return none.
    pub static ref TASK_MANAGER: TaskManager = TaskManager::new();
}

pub struct TaskManager(SpinNoIrqLock<BTreeMap<Tid, Weak<Task>>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_task(&self, task: &Arc<Task>) {
        // println!("[add_task] {}", task.tid());
        self.0.lock().insert(task.tid(), Arc::downgrade(task));
    }

    pub fn remove_task(&self, tid: Tid) {
        // println!("[remove_task] {tid}");
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

/// add task to the global Task Manager
pub fn add_task(task: &Arc<Task>) {
    TASK_MANAGER.add_task(task);
}

/// remove task from the global Task Manager
pub fn remove_task(tid: Tid) {
    TASK_MANAGER.remove_task(tid);
}

/// get task from the global Task Manager with an Option result.
pub fn get_task(tid: Tid) -> Option<Arc<Task>> {
    TASK_MANAGER.get_task(tid)
}
