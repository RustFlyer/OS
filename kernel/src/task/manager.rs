use crate::task::Task;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::sync::Weak;
use mutex::SpinNoIrqLock;
use systype::error::SysResult;

use crate::task::tid::Tid;

/// `TASK_MANAGER` takes control of all the user tasks in the kernel, using a BTreeMap
/// struct as the tid is the key and the weak point to task is the value.
/// Task Manager is a global struct which is shared with all harts. Therefore, it's
/// guarded by a spin lock that disables the interrupt.
///
/// When a task is spawned, it should be added into the `TASK_MANAGER`. Also, the task should
/// be removed when it is terminated.
///
/// `TASK_MANAGER` should not affect lifetime of a task so it does not ensure whether the task
/// is alive or not. When the task is terminated, `get_task` will return none.
pub static TASK_MANAGER: TaskManager = TaskManager::new();

pub struct TaskManager(SpinNoIrqLock<BTreeMap<Tid, Weak<Task>>>);

impl TaskManager {
    pub const fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_task(&self, task: &Arc<Task>) {
        self.0.lock().insert(task.tid(), Arc::downgrade(task));

        log::debug!("Add task {}", task.tid());
        log::debug!("Task list:");
        let _ = self.for_each(|t| {
            let t = t.tid();
            // simdebug::stop();
            // log::debug!(
            //     "thread {}, name: {}, state: {:?}, page table at {:#x}",
            //     t.tid(),
            //     t.get_name(),
            //     t.get_state(),
            //     t.addr_space().page_table.root().to_usize(),
            // );
            Ok(())
        });
    }

    pub fn remove_task(&self, tid: Tid) {
        self.0.lock().remove(&tid);
        log::debug!("[remove_task] {tid}");
        // self.for_each(|t| {
        //     log::debug!(
        //         "thread {}, name: {}, state: {:?}",
        //         t.tid(),
        //         t.get_name(),
        //         t.get_state()
        //     );
        //     Ok(())
        // })
        // .unwrap();
    }

    pub fn get_task(&self, tid: Tid) -> Option<Arc<Task>> {
        self.0
            .lock()
            .get(&tid)
            .and_then(|weak_task| weak_task.upgrade())
    }

    pub fn for_each(&self, f: impl Fn(&Arc<Task>) -> SysResult<()>) -> SysResult<()> {
        let tasks = self.0.lock();
        for task in tasks.values() {
            if let Some(task) = task.upgrade() {
                f(&task)?
            }
        }
        Ok(())
    }

    pub fn inner(&self) -> &SpinNoIrqLock<BTreeMap<Tid, Weak<Task>>> {
        &self.0
    }

    pub fn how_many_tasks(&self) -> usize {
        self.0.lock().len()
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
