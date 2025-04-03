extern crate alloc;
use alloc::sync::Arc;

use systype::SysError;
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
        log::debug!("t");
        let meta = SuperBlockMeta::new(dev, self.clone());
        log::debug!("t1");
        let superblock = ExtSuperBlock::new(meta);
        log::debug!("t12");
        let root_dir = ExtDir::open("/").map_err(SysError::from_i32)?;
        let root_inode = ExtDirInode::new(superblock.clone(), root_dir);
        let root_dentry = ExtDentry::new(name, superblock.clone(), parent.clone());
        root_dentry.set_inode(root_inode);
        log::debug!("t123");

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
