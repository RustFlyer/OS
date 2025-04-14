extern crate alloc;

use super::SleepCASLock;
use alloc::sync::Arc;
use osfuture::yield_now;

pub type OptimisticLock<T> = Arc<SleepCASLock<T>>;

pub fn new_optimistic_mutex<T>(data: T) -> OptimisticLock<T> {
    Arc::new(SleepCASLock::new(data))
}

pub async fn optimistic_mutex_test() {
    pub struct Apple {
        pub bug: usize,
    }
    let locka = new_optimistic_mutex(Apple { bug: 0 });
    {
        let mut apple = locka.lock().await;
        apple.bug = 9;
        yield_now().await;
        apple.bug = 8;
    }
    {
        let apple = locka.lock().await;
        log::info!("apple bug = {}", apple.bug);
        yield_now().await;
        log::info!("apple bug = {}", apple.bug);
    }
}
