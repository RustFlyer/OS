use alloc::sync::Arc;
use config::{
    inode::InodeMode,
    vfs::{MountFlags, StatFs},
};
use driver::BlockDevice;
use systype::SysResult;
use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    inode::Inode,
    superblock::{SuperBlock, SuperBlockMeta},
};

use crate::simple::{dentry::SimpleDentry, inode::SimpleInode};

pub mod stdio;
pub mod tty;

pub struct DevFsType {
    meta: FileSystemTypeMeta,
}

impl DevFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("devfs"),
        })
    }
}

impl FileSystemType for DevFsType {
    fn get_meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let sb = DevSuperBlock::new(dev, self.clone());
        let mount_inode = SimpleInode::new(sb.clone());
        {
            mount_inode.get_meta().inner.lock().mode = InodeMode::DIR;
        }
        let parentv = if let Some(p) = parent.clone() {
            Some(Arc::downgrade(&p))
        } else {
            None
        };
        let mount_dentry = SimpleDentry::new(name, Some(mount_inode), parentv);
        if let Some(parent) = parent {
            parent.add_child(mount_dentry.clone());
        }
        self.insert_sblk(&mount_dentry.path(), sb);
        Ok(mount_dentry)
    }

    fn kill_sblk(&self, _sblk: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}

struct DevSuperBlock {
    meta: SuperBlockMeta,
}

impl DevSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for DevSuperBlock {
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
