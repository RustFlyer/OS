extern crate alloc;

use alloc::sync::Arc;
use config::vfs::StatFs;
use lwext4_rust::Ext4BlockWrapper;
use systype::SysResult;
use vfs::superblock::{SuperBlock, SuperBlockMeta};

use crate::disk::Disk;

pub struct ExtSuperBlock {
    meta: SuperBlockMeta,
    inner: Ext4BlockWrapper<Disk>,
}

unsafe impl Sync for ExtSuperBlock {}
unsafe impl Send for ExtSuperBlock {}

impl ExtSuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let disk = Disk::new(meta.device.as_ref().unwrap().clone());
        Arc::new(Self {
            meta: meta,
            inner: Ext4BlockWrapper::<Disk>::new(disk).unwrap(),
        })
    }
}

impl SuperBlock for ExtSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        todo!()
    }

    fn sync_fs(&self, wait: isize) -> SysResult<()> {
        todo!()
    }
}
