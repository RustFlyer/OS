use alloc::sync::{Arc, Weak};
use config::inode::InodeMode;
use systype::error::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::file::RtcFile;

pub struct RtcDentry {
    meta: DentryMeta,
}

impl RtcDentry {
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

impl Dentry for RtcDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
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

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(RtcFile::new(self.clone()))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }
}
