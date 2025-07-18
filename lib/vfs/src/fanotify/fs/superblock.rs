use alloc::sync::Arc;

use lazy_static::lazy_static;

use config::vfs::StatFs;
use driver::BlockDevice;
use systype::error::SysResult;

use crate::{
    fstype::FileSystemType,
    superblock::{SuperBlock, SuperBlockMeta},
};

use super::filesystem::FILE_SYSTEM_TYPE;

lazy_static! {
    /// The superblock for fanotify files.
    pub static ref SUPERBLOCK: Arc<FanotifyEventFileSuperBlock>
        = Arc::new(FanotifyEventFileSuperBlock::new(None, FILE_SYSTEM_TYPE.clone()));
}

pub struct FanotifyEventFileSuperBlock {
    /// The superblock associated with the fanotify event file.
    meta: SuperBlockMeta,
}

impl FanotifyEventFileSuperBlock {
    /// Creates a new fanotify event file superblock.
    pub fn new(device: Option<Arc<dyn BlockDevice>>, fs_type: Arc<dyn FileSystemType>) -> Self {
        let meta = SuperBlockMeta::new(device, fs_type);
        Self { meta }
    }
}

impl SuperBlock for FanotifyEventFileSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        unimplemented!()
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        unimplemented!()
    }
}
