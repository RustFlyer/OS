use core::sync::atomic::AtomicUsize;

use alloc::sync::{Arc, Weak};
use hashbrown::HashMap;
use mutex::SpinNoIrqLock;

use crate::task::Task;
use spin::Once;

use core::sync::atomic::Ordering::Relaxed as R5d;

struct PFInfo {
    pidfd: usize,
    task: Weak<Task>,
}

pub struct PFTable {
    table: SpinNoIrqLock<HashMap<usize, PFInfo>>,
    next_pidfd: AtomicUsize,
}

pub static PF_TABLE: Once<PFTable> = Once::new();

pub fn init_pf_table() {
    PF_TABLE.call_once(|| PFTable::new());
}

impl PFTable {
    pub fn new() -> Self {
        Self {
            table: SpinNoIrqLock::new(HashMap::new()),
            next_pidfd: AtomicUsize::new(100),
        }
    }

    pub fn new_pidfd(&self, task: &Arc<Task>) -> usize {
        let pidfd = self.next_pidfd.load(R5d);
        let mut table = self.table.lock();
        table.insert(pidfd, PFInfo {
            pidfd,
            task: Arc::downgrade(task),
        });
        self.next_pidfd.store(pidfd + 1, R5d);
        pidfd
    }

    pub fn get_task_by_pidfd(&self, pidfd: usize) -> Option<Arc<Task>> {
        let table = self.table.lock();
        table.get(&pidfd)?.task.upgrade()
    }

    pub fn remove_pidfd(&self, pidfd: usize) {
        let mut table = self.table.lock();
        table.remove(&pidfd);
    }
}
