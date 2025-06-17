use alloc::sync::{Arc, Weak};
use systype::error::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::file::NullFile;

pub struct NullDentry {
    meta: DentryMeta,
}

impl NullDentry {
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

impl Dentry for NullDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(NullFile {
            meta: FileMeta::new(self),
        }))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: config::inode::InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
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

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_rmdir_recur(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_symlink(&self, _dentry: &dyn Dentry, _target: &str) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }
}
