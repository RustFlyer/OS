use id_allocator::{IdAllocator, VecIdAllocator};
use lazy_static::lazy_static;
use mutex::SpinNoIrqLock;

lazy_static! {
    static ref TIMERID_ALLOCATOR: SpinNoIrqLock<VecIdAllocator> =
        SpinNoIrqLock::new(VecIdAllocator::new(0, usize::MAX));
}

#[derive(Debug)]
pub struct TimerHandle(pub usize);

impl Drop for TimerHandle {
    fn drop(&mut self) {
        unsafe { TIMERID_ALLOCATOR.lock().dealloc(self.0) };
    }
}

/// `timeid_alloc()` can look for an unused id from 0
/// and allocate it for a new timer.
pub fn timeid_alloc() -> TimerHandle {
    match TIMERID_ALLOCATOR.lock().alloc() {
        Some(tid) => TimerHandle(tid),
        None => panic!("no more TIDs available"),
    }
}
