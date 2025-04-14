extern crate alloc;

use super::SleepLock;
use alloc::sync::Arc;

pub type OptimisticLock<T> = Arc<SleepLock<T>>;

pub fn new_optimistic_mutex<T>(data: T) -> OptimisticLock<T> {
    Arc::new(SleepLock::new(data))
}
