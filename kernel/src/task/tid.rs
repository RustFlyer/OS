use config::process::INIT_PROC_ID;
use id_allocator::{IdAllocator, VecIdAllocator};
use lazy_static::lazy_static;
use mutex::SpinNoIrqLock;

type TidAllocator = VecIdAllocator;

lazy_static! {
    static ref TID_ALLOCATOR: SpinNoIrqLock<TidAllocator> =
        SpinNoIrqLock::new(TidAllocator::new(INIT_PROC_ID, usize::MAX));
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

/// `tid_alloc_incr()` can look for an increment id from [`INIT_PROC_ID`]
/// and allocate it for a new thread.
pub fn tid_alloc_incr() -> TidHandle {
    match TID_ALLOCATOR.lock().alloc_incr() {
        Some(tid) => TidHandle(tid),
        None => panic!("no more TIDs available"),
    }
}

/// Attributes which are set by `set_tid_address` syscall, or when calling `clone` syscall
/// with `CLONE_CHILD_SETTID` or `CLONE_CHILD_CLEARTID` flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TidAddress {
    /// This attribute is set when this thread calls `clone` syscall with
    /// `CLONE_CHILD_SETTID` flag. If it is set, the kernel will write the child's TID to
    /// this address before giving control to the child thread. This address should be in
    /// the child's memory.
    pub set_child_tid: Option<usize>,

    /// This attribute is set when this thread calls `clone` syscall with
    /// `CLONE_CHILD_CLEARTID` flag, or when it calls `set_tid_address` syscall. If it is
    /// set, the kernel will clear the child's TID (set it to zero) at this address when
    /// the child thread exits, and wake up a single thread that is waiting on the futex
    /// at this address.
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
