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

use super::group::dentry::FanotifyGroupDentry;

lazy_static! {
    /// The filesystem for fanotify files.
    pub static ref FILE_SYSTEM_TYPE: Arc<dyn FileSystemType> = FanotifyGroupFsType::new();
}

pub struct FanotifyGroupFsType {
    meta: FileSystemTypeMeta,
}

impl FanotifyGroupFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("fanotify_eventfs"),
        })
    }
}

impl FileSystemType for FanotifyGroupFsType {
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
        Ok(FanotifyGroupDentry::new(None))
    }

    fn kill_sblk(&self, _sblk: Arc<dyn SuperBlock>) -> SysResult<()> {
        unimplemented!()
    }
}
