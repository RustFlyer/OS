use alloc::sync::Arc;
use config::vfs::StatFs;
use driver::BlockDevice;
use systype::error::SysResult;
use vfs::{
    fstype::FileSystemType,
    superblock::{SuperBlock, SuperBlockMeta},
};

pub struct ProcSuperBlock {
    meta: SuperBlockMeta,
}

impl ProcSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type, 0x88),
        })
    }
}

impl SuperBlock for ProcSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        todo!()
    }
}
