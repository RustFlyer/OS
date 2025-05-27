use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use mutex::SpinNoIrqLock;

use super::{Task, tid::PGid};

/// `PROCESS_GROUP_MANAGER` controls groups with processes as a basic unit.
/// It organizes datas in the form of BTreeMap. The key is PGid, also the Pid
/// of the leading process. And the value is a vector of weak pointers to tasks.
/// User can find members of a group by the leading process Pid.
///
/// As Task Manager, `PROCESS_GROUP_MANAGER` is also a global variable, which
/// means that it may be snatched by different harts in the same time. Therefore,
/// it is locked by a spin lock and should be used by one hart in one time.
///
/// `PROCESS_GROUP_MANAGER` should not affect the lifetime of task. All of pointers
/// to task is weak and `PROCESS_GROUP_MANAGER` does not guarantee all the pointers
/// are valid.  
pub static PROCESS_GROUP_MANAGER: ProcessGroupManager = ProcessGroupManager::new();

pub struct ProcessGroupManager(SpinNoIrqLock<BTreeMap<PGid, Vec<Weak<Task>>>>);

impl ProcessGroupManager {
    pub const fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    /// add group leader process and the `Pid` of leader process is
    /// inserted in `ProcessGroupManager` as a key.
    pub fn add_group(&self, group_leader: &Arc<Task>) {
        let pgid = group_leader.tid();
        group_leader.set_pgid(pgid);
        let group = vec![Arc::downgrade(group_leader)];
        self.0.lock().insert(pgid, group);
    }

    /// adds a process in a existed group.
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

    /// gets group by Pid of leader process but the result is not
    /// guaranteed to exist.
    pub fn get_group(&self, pgid: PGid) -> Option<Vec<Weak<Task>>> {
        self.0.lock().get(&pgid).cloned()
    }

    /// removes a process group by its leader task.
    pub fn remove(&self, process: &Arc<Task>) {
        if self.0.lock().get_mut(&process.get_pgid()).is_none() {
            return;
        }
        self.0
            .lock()
            .get_mut(&process.get_pgid())
            .unwrap()
            .retain(|task| {
                task.upgrade()
                    .filter(|t| !Arc::ptr_eq(process, t))
                    .is_some()
            })
    }
}
