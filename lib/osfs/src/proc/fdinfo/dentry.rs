use alloc::{
    format,
    sync::{Arc, Weak},
};
use config::inode::InodeMode;
use systype::error::SysResult;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::{file::FdInfoFile, inode::FdInfoInode};

pub struct FdInfoDentry {
    meta: DentryMeta,
}

impl FdInfoDentry {
    pub fn new(
        fd: usize,
        inode: Option<Arc<FdInfoInode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        let name = format!("{}", fd);
        Arc::new(Self {
            meta: DentryMeta::new(&name, inode.map(|i| i as Arc<dyn Inode>), parent),
        })
    }
}

impl Dentry for FdInfoDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(FdInfoFile {
            meta: FileMeta::new(self),
        }))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::ENOTDIR)
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(systype::error::SysError::EACCES)
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        panic!("FdInfoDentry does not support new_neg_child")
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
