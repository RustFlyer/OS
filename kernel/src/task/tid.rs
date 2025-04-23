use config::process::INIT_PROC_ID;
use id_allocator::{IdAllocator, VecIdAllocator};
use lazy_static::lazy_static;
use mutex::SpinLock;

type TidAllocator = VecIdAllocator;

lazy_static! {
    static ref TID_ALLOCATOR: SpinLock<TidAllocator> =
        SpinLock::new(TidAllocator::new(INIT_PROC_ID, usize::MAX));
}

pub type Tid = usize;
pub type Pid = usize;
pub type PGid = usize;

#[derive(Debug)]
pub struct TidHandle(pub Tid);

impl Drop for TidHandle {
    fn drop(&mut self) {
        unsafe { TID_ALLOCATOR.lock().dealloc(self.0) };
    }
}

/// `tid_alloc()` can look for an unused id from [`INIT_PROC_ID`]
/// and allocate it for a new thread.
pub fn tid_alloc() -> TidHandle {
    match TID_ALLOCATOR.lock().alloc() {
        Some(tid) => TidHandle(tid),
        None => panic!("no more TIDs available"),
    }
}

/// Tid address which may be set by `set_tid_address` syscall.
pub struct TidAddress {
    /// When set, when spawning a new thread, the kernel sets the thread's tid
    /// to this address.
    pub set_child_tid: Option<usize>,
    /// When set, when the thread exits, the kernel sets the thread's tid to
    /// this address, and wake up a futex waiting on this address.
    pub clear_child_tid: Option<usize>,
}

impl TidAddress {
    pub const fn new() -> Self {
        Self {
            set_child_tid: None,
            clear_child_tid: None,
        }
    }
}
