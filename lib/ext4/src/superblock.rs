extern crate alloc;

use alloc::sync::Arc;
use config::vfs::StatFs;
use lwext4_rust::Ext4BlockWrapper;
use systype::error::SysResult;
use vfs::superblock::{SuperBlock, SuperBlockMeta};

use crate::disk::Disk;

#[allow(unused)]
pub struct ExtSuperBlock {
    meta: SuperBlockMeta,
    inner: Ext4BlockWrapper<Disk>,
}

unsafe impl Sync for ExtSuperBlock {}
unsafe impl Send for ExtSuperBlock {}

impl ExtSuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let dev = meta.device.as_ref().unwrap().clone();
        let disk = Disk::new(dev);
        log::debug!("try to initialize EXT4 filesystem");
        let inner =
            Ext4BlockWrapper::<Disk>::new(disk).expect("failed to initialize EXT4 filesystem");
        log::debug!("initialize EXT4 filesystem");
        Arc::new(Self { meta: meta, inner })
    }
}

impl SuperBlock for ExtSuperBlock {
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
