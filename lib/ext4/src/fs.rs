extern crate alloc;
use alloc::sync::Arc;

use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    superblock::{SuperBlock, SuperBlockMeta},
};

use crate::{
    dentry::ExtDentry, ext::dir::ExtDir, inode::dir::ExtDirInode, superblock::ExtSuperBlock,
};

pub struct ExtFsType {
    meta: FileSystemTypeMeta,
}

impl ExtFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("ext4"),
        })
    }
}

impl FileSystemType for ExtFsType {
    fn get_meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn vfs::dentry::Dentry>>,
        flags: config::vfs::MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs::dentry::Dentry>> {
        let superblock = ExtSuperBlock::new(SuperBlockMeta::new(devs, self.clone()));
        let root_dir = ExtDir::open("/").map_err(SysError::from_i32)?;
        let root_inode = ExtDirInode::new(superblock.clone(), root_dir);
        let root_dentry = ExtDentry::new(name, superblock.clone(), parent.clone());
        root_dentry.set_inode(root_inode);

        if let Some(parent) = parent {
            parent.insert(root_dentry.clone());
        }

        superblock.set_root_dentry(root_dentry.clone());
        self.insert_sblk(&root_dentry.path(), superblock);
        Ok(root_dentry)
    }

    fn kill_sblk(&self, sblk: Arc<dyn SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}
