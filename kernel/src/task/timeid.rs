use lazy_static::lazy_static;

use mutex::SpinNoIrqLock;

lazy_static! {
    static ref TIMERID_ALLOCATOR: SpinNoIrqLock<IncreasingAllocator> =
        SpinNoIrqLock::new(IncreasingAllocator::new());
}

#[derive(Debug)]
pub struct TimerHandle(pub usize);

impl Drop for TimerHandle {
    fn drop(&mut self) {
        // No-op: IDs are not reused in this simple allocator
    }
}

struct IncreasingAllocator {
    next_id: usize,
}

impl IncreasingAllocator {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn alloc(&mut self) -> usize {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

/// `timeid_alloc()` allocates a new id by incrementing the last id.
pub fn timeid_alloc() -> TimerHandle {
    let tid = TIMERID_ALLOCATOR.lock().alloc();
    TimerHandle(tid)
}
