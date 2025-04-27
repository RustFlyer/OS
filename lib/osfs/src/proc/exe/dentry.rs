use alloc::sync::{Arc, Weak};
use systype::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::file::ExeFile;

pub struct ExeDentry {
    meta: DentryMeta,
}

impl ExeDentry {
    pub fn new(inode: Option<Arc<dyn Inode>>, parent: Option<Weak<dyn Dentry>>) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new("exe", inode, parent),
        })
    }
}

impl Dentry for ExeDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(ExeFile {
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

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
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
}
