use id_allocator::IdAllocator;
use lazy_static::lazy_static;
use mutex::SpinNoIrqLock;

lazy_static! {
    static ref TID_ALLOCATOR: SpinNoIrqLock<IdAllocator> = SpinNoIrqLock::new(IdAllocator::new());
}

pub type Tid = usize;
pub type Pid = usize;
pub type PGid = usize;

#[derive(Debug)]
pub struct TidHandle(pub Tid);

impl Drop for TidHandle {
    fn drop(&mut self) {
        TID_ALLOCATOR.lock().dealloc(self.0);
    }
}

pub fn tid_alloc() -> TidHandle {
    TidHandle(TID_ALLOCATOR.lock().alloc())
}
