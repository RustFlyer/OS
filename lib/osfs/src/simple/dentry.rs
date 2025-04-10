use alloc::sync::{Arc, Weak};
use config::inode::{InodeMode, InodeType};
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::file::SimpleDirFile;

pub struct SimpleDentry {
    meta: DentryMeta,
}

impl SimpleDentry {
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
}

impl Dentry for SimpleDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
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

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self.inode().ok_or(SysError::EEXIST)?;
        match inode.inotype() {
            InodeType::Dir => Ok(SimpleDirFile::new(self.clone())),
            _ => unreachable!(),
        }
    }

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }
}
