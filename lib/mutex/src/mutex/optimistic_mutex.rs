extern crate alloc;

use super::SleepCASLock;
use alloc::sync::Arc;

pub type OptimisticLock<T> = Arc<SleepCASLock<T>>;

pub fn new_optimistic_mutex<T>(data: T) -> OptimisticLock<T> {
    Arc::new(SleepCASLock::new(data))
}
