use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use config::vfs::StatFs;
use driver::BlockDevice;
use mutex::SpinNoIrqLock;
use spin::Once;
use systype::error::SysResult;

use crate::{dentry::Dentry, fanotify::FanotifyEntry, fstype::FileSystemType};

static _VIRTUAL_DEV_COUNTER: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0x1000);

pub struct SuperBlockMeta {
    pub device: Option<Arc<dyn BlockDevice>>,
    pub dev_id: u64,
    pub fs_type: Arc<dyn FileSystemType>,
    pub root_dentry: Once<Arc<dyn Dentry>>,
    pub fanotify_entries: SpinNoIrqLock<Vec<Weak<FanotifyEntry>>>,
}

impl SuperBlockMeta {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
        dev_id: u64,
    ) -> Self {
        // let dev_id = VIRTUAL_DEV_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        Self {
            device,
            dev_id,
            fs_type,
            root_dentry: Once::new(),
            fanotify_entries: SpinNoIrqLock::new(Vec::new()),
        }
    }
}

pub trait SuperBlock: Send + Sync {
    fn meta(&self) -> &SuperBlockMeta;

    fn stat_fs(&self) -> SysResult<StatFs>;

    fn sync_fs(&self, wait: isize) -> SysResult<()>;

    fn set_root_dentry(&self, root_dentry: Arc<dyn Dentry>) {
        self.meta().root_dentry.call_once(|| root_dentry);
    }

    fn dev_id(&self) -> u64 {
        self.meta().dev_id
    }
}

impl dyn SuperBlock {
    /// Returns the file system type of this super block.
    pub fn fs_type(&self) -> Arc<dyn FileSystemType> {
        Arc::clone(&self.meta().fs_type)
    }

    /// Returns the root dentry.
    pub fn root_dentry(&self) -> Arc<dyn Dentry> {
        Arc::clone(self.meta().root_dentry.get().unwrap())
    }

    /// Returns the device associated with this super block.
    pub fn device(&self) -> Option<Arc<dyn BlockDevice>> {
        self.meta().device.as_ref().cloned()
    }
}
