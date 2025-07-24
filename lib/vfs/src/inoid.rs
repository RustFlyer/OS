use core::sync::atomic::{AtomicI32, Ordering};

static INODE_NUMBER: AtomicI32 = AtomicI32::new(0);

pub fn alloc_ino() -> i32 {
    INODE_NUMBER.fetch_add(1, Ordering::Relaxed)
}
