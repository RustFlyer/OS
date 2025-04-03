extern crate alloc;

use core::{ffi::c_void, ptr::null_mut};

use alloc::{boxed::Box, ffi::CString, sync::Arc};
use config::vfs::StatFs;
use log::info;
use lwext4_rust::{Ext4BlockWrapper, KernelDevOp};
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
        let dev = meta.device.as_ref().unwrap().clone();
        let disk = Disk::new(dev);
        log::debug!("b12");
        let inner =
            Ext4BlockWrapper::<Disk>::new(disk).expect("failed to initialize EXT4 filesystem");
        log::debug!("b123");
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

    fn sync_fs(&self, wait: isize) -> SysResult<()> {
        todo!()
    }
}
