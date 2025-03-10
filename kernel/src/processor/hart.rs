use crate::task::Task;
use crate::task::future::FutureContext;
use config::device::MAX_HARTS;

extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref HARTS: Vec<Arc<HART>> = (0..MAX_HARTS).map(|_| Arc::new(HART::new())).collect();
}

pub struct HART {
    pub id: usize,
    task: Option<Arc<Task>>,
    context: FutureContext,
}

impl HART {
    pub fn new() -> Self {
        Self {
            id: 0,
            task: None,
            context: FutureContext::new(),
        }
    }
}
