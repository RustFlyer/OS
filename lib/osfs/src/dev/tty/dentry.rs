use alloc::sync::{Arc, Weak};
use config::inode::InodeMode;
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::file::TtyFile;

pub struct TtyDentry {
    meta: DentryMeta,
}

impl TtyDentry {
    pub fn new(
        name: &str,
        inode: Option<Arc<dyn Inode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, inode, parent),
        })
    }
}

impl Dentry for TtyDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(TtyFile::new(self.clone()))
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn set_inode(&self, inode: Arc<dyn Inode>) {
        *self.meta.inode.lock() = Some(inode);
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        todo!()
    }
}
