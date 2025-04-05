use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use config::vfs::StatFs;
use driver::BlockDevice;
use mutex::SpinNoIrqLock;
use spin::Once;
use systype::SysResult;

use crate::{dentry::Dentry, fstype::FileSystemType, inode::Inode};

pub struct SuperBlockMeta {
    pub device: Option<Arc<dyn BlockDevice>>,
    pub fs_type: Weak<dyn FileSystemType>,
    pub root_dentry: Once<Arc<dyn Dentry>>,

    pub inodes: SpinNoIrqLock<Vec<Arc<dyn Inode>>>,
    pub dirty_inodes: SpinNoIrqLock<Vec<Arc<dyn Inode>>>,
}

impl SuperBlockMeta {
    pub fn new(device: Option<Arc<dyn BlockDevice>>, fs_type: Arc<dyn FileSystemType>) -> Self {
        Self {
            device,
            root_dentry: Once::new(),
            fs_type: Arc::downgrade(&fs_type),
            inodes: SpinNoIrqLock::new(Vec::new()),
            dirty_inodes: SpinNoIrqLock::new(Vec::new()),
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
}

impl dyn SuperBlock {
    pub fn fs_type(&self) -> Arc<dyn FileSystemType> {
        self.meta().fs_type.upgrade().unwrap()
    }

    pub fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta().root_dentry.get().unwrap().clone()
    }

    pub fn push_inode(&self, inode: Arc<dyn Inode>) {
        self.meta().inodes.lock().push(inode)
    }
}
