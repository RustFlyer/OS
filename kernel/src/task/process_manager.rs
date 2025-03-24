use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use mutex::SpinNoIrqLock;

use super::{Task, tid::PGid};

pub static PROCESS_GROUP_MANAGER: ProcessGroupManager = ProcessGroupManager::new();

pub struct ProcessGroupManager(SpinNoIrqLock<BTreeMap<PGid, Vec<Weak<Task>>>>);

impl ProcessGroupManager {
    pub const fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_group(&self, group_leader: &Arc<Task>) {
        let pgid = group_leader.tid();
        group_leader.set_pgid(pgid);
        let mut group = Vec::new();
        group.push(Arc::downgrade(group_leader));
        self.0.lock().insert(pgid, group);
    }

    pub fn add_process(&self, pgid: PGid, process: &Arc<Task>) {
        if !process.is_process() {
            log::warn!("[ProcessGroupManager::add_process] try adding task that is not a process");
            return;
        }
        process.set_pgid(pgid);
        let mut inner = self.0.lock();
        log::info!("pgid is [{}]", pgid);
        let vec = inner.get_mut(&pgid).unwrap();
        vec.push(Arc::downgrade(process));
    }

    pub fn get_group(&self, pgid: PGid) -> Option<Vec<Weak<Task>>> {
        self.0.lock().get(&pgid).cloned()
    }

    pub fn remove(&self, process: &Arc<Task>) {
        self.0
            .lock()
            .get_mut(&process.get_pgid())
            .unwrap()
            .retain(|task| task.upgrade().map_or(false, |t| !Arc::ptr_eq(process, &t)))
    }
}
