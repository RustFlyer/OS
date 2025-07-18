use alloc::sync::Arc;

use config::vfs::MountFlags;
use driver::BlockDevice;
use systype::error::SysResult;

use lazy_static::lazy_static;

use crate::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    superblock::SuperBlock,
};

use super::event::dentry::FanotifyEventDentry;

lazy_static! {
    /// The filesystem for fanotify files.
    pub static ref FILE_SYSTEM_TYPE: Arc<dyn FileSystemType> = FanotifyEventFsType::new();
}

pub struct FanotifyEventFsType {
    meta: FileSystemTypeMeta,
}

impl FanotifyEventFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("fanotify_eventfs"),
        })
    }
}

impl FileSystemType for FanotifyEventFsType {
    fn get_meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        _name: &str,
        _parent: Option<Arc<dyn Dentry>>,
        _flags: MountFlags,
        _dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        Ok(FanotifyEventDentry::new(None, None))
    }

    fn kill_sblk(&self, _sblk: Arc<dyn SuperBlock>) -> SysResult<()> {
        unimplemented!()
    }
}
