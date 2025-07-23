use alloc::sync::{Arc, Weak};
use config::inode::InodeMode;
use systype::error::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::file::MemFile;

pub struct MemDentry {
    meta: DentryMeta,
}

impl MemDentry {
    pub fn new(
        name: &str,
        inode: Option<Arc<dyn Inode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, inode, parent),
        })
    }

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }

    pub fn into_dyn_ref(&self) -> &dyn Dentry {
        self
    }
}

impl Dentry for MemDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(MemFile::new(self.clone()))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }
}
