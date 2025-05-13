use alloc::sync::{Arc, Weak};
use systype::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::file::ZeroFile;

pub struct ZeroDentry {
    meta: DentryMeta,
}

impl ZeroDentry {
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

impl Dentry for ZeroDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(ZeroFile {
            meta: FileMeta::new(self),
        }))
    }

    fn base_create(&self, dentry: &dyn Dentry, mode: config::inode::InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_link(&self, dentry: &dyn Dentry, old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn base_rename(
        &self,
        dentry: &dyn Dentry,
        new_dir: &dyn Dentry,
        new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        todo!()
    }

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_rmdir_recur(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_symlink(&self, _dentry: &dyn Dentry, _target: &str) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }
}
