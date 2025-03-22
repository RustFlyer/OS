extern crate alloc;

use super::SpinNoIrqLock;
use alloc::sync::Arc;

pub type ShareMutex<T> = Arc<SpinNoIrqLock<T>>;

pub fn new_share_mutex<T>(data: T) -> ShareMutex<T> {
    Arc::new(SpinNoIrqLock::new(data))
}
