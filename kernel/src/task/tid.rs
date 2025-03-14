use id_allocator::{IdAllocator, VecIdAllocator};
use lazy_static::lazy_static;
use mutex::SpinNoIrqLock;

type TidAllocator = VecIdAllocator;

lazy_static! {
    static ref TID_ALLOCATOR: SpinNoIrqLock<TidAllocator> =
        SpinNoIrqLock::new(TidAllocator::new(0, usize::MAX));
}

pub type Tid = usize;
pub type Pid = usize;
pub type PGid = usize;

/// RAII形式任务ID句柄结构体
///
/// 表示一个任务的ID，并实现Drop特性
#[derive(Debug)]
pub struct TidHandle(pub Tid);

impl Drop for TidHandle {
    fn drop(&mut self) {
        unsafe { TID_ALLOCATOR.lock().dealloc(self.0) };
    }
}

pub fn tid_alloc() -> TidHandle {
    match TID_ALLOCATOR.lock().alloc() {
        Some(tid) => TidHandle(tid),
        None => panic!("no more TIDs available"),
    }
}
