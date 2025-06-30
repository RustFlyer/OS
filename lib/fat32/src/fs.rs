use alloc::{boxed::Box, sync::Arc};
use config::vfs::MountFlags;
use driver::BlockDevice;
use systype::error::SysResult;
use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    superblock::{SuperBlock, SuperBlockMeta},
};

use crate::{dentry::FatDentry, inode::dir::FatDirInode, superblock::FatSuperBlock};

pub struct FatFsType {
    meta: FileSystemTypeMeta,
}

impl FatFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("fat32"),
        })
    }
}

impl FileSystemType for FatFsType {
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
        debug_assert!(dev.is_some());
        let sb = FatSuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let sblk = sb.clone();
        let sb = Box::leak(Box::new(sb));

        let root_inode = FatDirInode::new(sb.clone(), sb.fs.root_dir());
        let wparent = if let Some(p) = parent.clone() {
            Some(Arc::downgrade(&p))
        } else {
            None
        };
        let root_dentry = FatDentry::new(name, Some(root_inode), wparent).into_dyn();

        if let Some(parent) = parent {
            parent.add_child(root_dentry.clone());
        }

        sb.set_root_dentry(root_dentry.clone());
        self.insert_sblk(&root_dentry.path(), sblk);
        Ok(root_dentry)
    }

    fn kill_sblk(&self, _sb: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}
