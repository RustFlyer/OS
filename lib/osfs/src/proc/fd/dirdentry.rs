use alloc::sync::{Arc, Weak};
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
    superblock::SuperBlock,
};

use crate::simple::inode::SimpleInode;

use super::{dentry::FdDentry, inode::FdInode};

/// FdDirDentry can generate sub-dentry when lookup
///
/// but something goes wrong...
pub struct FdDirDentry {
    meta: DentryMeta,
}

impl FdDirDentry {
    pub fn new(parent: Option<Weak<dyn Dentry>>, superblock: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = SimpleInode::new(superblock);
        inode.set_inotype(config::inode::InodeType::Dir);
        Arc::new(Self {
            meta: DentryMeta::new("fd", Some(inode), parent),
        })
    }

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }

    pub fn into_dyn_ref(&self) -> &dyn Dentry {
        self
    }
}

impl Dentry for FdDirDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        todo!()
    }

    fn base_create(&self, dentry: &dyn Dentry, _mode: config::inode::InodeMode) -> SysResult<()> {
        todo!();
        let name = dentry.name();
        if let Ok(n) = name.parse::<u64>() {
            let fdinode = FdInode::new(self.superblock().unwrap(), n as usize);
            dentry.set_inode(fdinode);
            Ok(())
        } else {
            Err(SysError::ENOENT)
        }
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let name = dentry.name();
        let _ = self.get_child(name).ok_or(SysError::ENOENT)?;

        Ok(())
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        todo!();
        let n = name.parse::<u64>().unwrap_or(0);
        let dentry = FdDentry::new(None, Some(Arc::downgrade(&self.into_dyn())), n as usize);
        dentry
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }
}
