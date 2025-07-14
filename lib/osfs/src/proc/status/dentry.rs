use alloc::sync::{Arc, Weak};
use systype::error::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::file::StatusFile;

pub struct StatusDentry {
    meta: DentryMeta,
}

impl StatusDentry {
    pub fn new(inode: Option<Arc<dyn Inode>>, parent: Option<Weak<dyn Dentry>>) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new("status", inode, parent),
        })
    }
}

impl Dentry for StatusDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(StatusFile {
            meta: FileMeta::new(self),
        }))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: config::inode::InodeMode) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::ENOTDIR)
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        panic!("StatusDentry does not support new_neg_child")
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
