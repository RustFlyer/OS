use alloc::sync::Arc;
use config::{inode::InodeType, vfs::MountFlags};
use driver::BlockDevice;
use systype::SysResult;
use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    inode::Inode,
    superblock::SuperBlock,
};

use crate::simple::{dentry::SimpleDentry, inode::SimpleInode};

use super::superblock::ProcSuperBlock;

pub struct ProcFsType {
    meta: FileSystemTypeMeta,
}

impl ProcFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("procfs"),
        })
    }
}

impl FileSystemType for ProcFsType {
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
        let sb = ProcSuperBlock::new(dev, self.clone());
        let mount_inode = SimpleInode::new(sb.clone());
        mount_inode.set_size(0);
        mount_inode.set_inotype(InodeType::Dir);
        let mount_dentry = SimpleDentry::new(
            name,
            Some(mount_inode.clone()),
            parent.clone().map(|d| Arc::downgrade(&d)),
        );
        mount_dentry.set_inode(mount_inode.clone());
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
