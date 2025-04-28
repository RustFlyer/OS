use alloc::sync::Arc;
use mutex::SpinNoIrqLock;
use spin::Lazy;
use vfs::dentry::Dentry;

pub mod dentry;
pub mod file;
pub mod inode;

pub static KERNEL_TASK_DENTRYS: Lazy<[SpinNoIrqLock<Option<Arc<dyn Dentry>>>; 4]> =
    Lazy::new(|| {
        [
            SpinNoIrqLock::new(None),
            SpinNoIrqLock::new(None),
            SpinNoIrqLock::new(None),
            SpinNoIrqLock::new(None),
        ]
    });
