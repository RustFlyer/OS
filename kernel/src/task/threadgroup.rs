use core::fmt::Debug;
use core::fmt::Formatter;

use super::{Task, tid::Tid};
use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
};

/// `ThreadGroup` controls groups with processes as a basic unit.
/// It organizes datas in the form of BTreeMap. The key is Tid and the value is a
/// weak pointer to a task.
///
/// Unlike TaskManager, `ThreadGroup` is not a global variable. It is a member
/// of any task, which means that a task can add other tasks in its thread group.
///
/// `ThreadGroup` should not affect the lifetime of task. All of pointers
/// to task is weak and `ThreadGroup` does not guarantee all the pointers
/// are valid.  
///
/// Attention: A `ThreadGroup` of a task should regard the task itself as a member
/// in the group. Therefore, a new task should add itself into its new thread group
/// when it is created.
pub struct ThreadGroup(BTreeMap<Tid, Weak<Task>>);

impl Debug for ThreadGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut s = f.debug_struct("ThreadGroup");
        for t in self.iter() {
            s.field("thread", &format_args!("{} ({})", t.get_name(), t.tid()));
        }
        s.finish()
    }
}

impl ThreadGroup {
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, task: Arc<Task>) {
        self.0.insert(task.tid(), Arc::downgrade(&task));
    }

    pub fn remove(&mut self, task: &Task) {
        self.0.remove(&task.tid());
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<Task>> + '_ {
        self.0.values().map(|t| t.upgrade().unwrap())
    }
}
