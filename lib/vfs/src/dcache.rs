use hashbrown::HashMap;
use mutex::SpinNoIrqLock;

extern crate alloc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Once;

use crate::dentry::Dentry;

pub static DCACHE: Once<DentryHashMap> = Once::new();

pub fn dcache() -> &'static DentryHashMap {
    DCACHE.call_once(DentryHashMap::new)
}
pub struct DentryHashMap(SpinNoIrqLock<HashMap<String, DentryBucket>>);

impl DentryHashMap {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }

    pub fn insert(&self, dentry: Arc<dyn Dentry>) {
        let name = dentry.name();
        let mut map = self.0.lock();

        if let Some(bucket) = map.get_mut(&name) {
            bucket.insert(dentry);
        } else {
            let mut bucket = DentryBucket::new();
            bucket.insert(dentry);
            map.insert(name, bucket);
        }
    }

    pub fn remove(&self, dentry: Arc<dyn Dentry>) {
        let name = dentry.name();
        let mut map = self.0.lock();
        if let Some(bucket) = map.get_mut(&name) {
            bucket.remove(&dentry);
        } else {
            log::warn!("no dentry to remove!");
        }
    }
}

pub struct DentryBucket(Vec<Arc<dyn Dentry>>);

impl DentryBucket {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, dentry: Arc<dyn Dentry>) {
        self.0.push(dentry);
    }

    pub fn remove(&mut self, dentry: &Arc<dyn Dentry>) {
        let index = self.0.iter().position(|x| Arc::ptr_eq(x, dentry)).unwrap();
        self.0.remove(index);
    }

    pub fn find_by_parent(&self, parent: &Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.0
            .iter()
            .find(|ddentry| Arc::ptr_eq(parent, &ddentry.parent().unwrap()))
            .cloned()
    }

    pub fn find_by_path(&self, path: &str) -> Option<Arc<dyn Dentry>> {
        self.0
            .iter()
            .find(|ddentry| ddentry.path() == path)
            .cloned()
    }
}
