use alloc::sync::Arc;

use config::{inode::InodeType, vfs::MountFlags};
use systype::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    fstype::{FileSystemType, FileSystemTypeMeta},
    inode::Inode,
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
        parent: Option<Arc<dyn Dentry>>,
        _flags: MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let meta = SuperBlockMeta::new(dev, self.clone());
        let superblock = ExtSuperBlock::new(meta);
        let root_dir = ExtDir::open("/").map_err(SysError::from_i32)?;
        let root_inode = ExtDirInode::new(superblock.clone(), root_dir);
        root_inode.set_inotype(InodeType::Dir);
        let root_dentry = ExtDentry::new(
            name,
            Some(root_inode.clone()),
            parent.as_ref().map(Arc::downgrade),
        );
        root_dentry.set_inode(root_inode);

        if let Some(parent) = parent {
            parent.add_child(root_dentry.clone());
        }

        superblock.set_root_dentry(root_dentry.clone());
        self.insert_sblk(&root_dentry.path(), superblock);
        Ok(root_dentry)
    }

    fn kill_sblk(&self, _sblk: Arc<dyn SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}
