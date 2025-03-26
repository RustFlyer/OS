use core::sync::atomic::{AtomicUsize, Ordering};

static INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

pub fn alloc_ino() -> usize {
    INODE_NUMBER.fetch_add(1, Ordering::Relaxed)
}
