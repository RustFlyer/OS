use core::fmt::Debug;

use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
};

use super::{Task, tid::Tid};

pub struct ThreadGroup(BTreeMap<Tid, Weak<Task>>);

impl Debug for ThreadGroup {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for t in self.iter() {
            let _ = write!(f, "thread [{}] id [{}]", t.get_name(), t.tid());
        }
        Ok(())
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
