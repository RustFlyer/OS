use alloc::sync::Arc;
use config::{
    inode::{InodeMode, InodeType},
    vfs::{MountFlags, StatFs},
};
use driver::BlockDevice;
use systype::error::SysResult;
use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    inode::Inode,
    superblock::{SuperBlock, SuperBlockMeta},
};

use crate::simple::{dentry::SimpleDentry, inode::SimpleInode};

pub struct VarFsType {
    meta: FileSystemTypeMeta,
}

impl VarFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("varfs"),
        })
    }
}

impl FileSystemType for VarFsType {
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
        let sb = VarSuperBlock::new(dev, self.clone());
        let mount_inode = SimpleInode::new(sb.clone());
        mount_inode.set_inotype(InodeType::from(InodeMode::DIR));

        let parentv = parent.clone().map(|p| Arc::downgrade(&p));

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

struct VarSuperBlock {
    meta: SuperBlockMeta,
}

impl VarSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type, 0x66),
        })
    }
}

impl SuperBlock for VarSuperBlock {
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
