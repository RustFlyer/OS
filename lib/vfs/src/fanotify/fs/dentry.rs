use alloc::sync::Arc;

use config::{inode::InodeMode, vfs::FileInternalFlags};
use systype::error::{SysError, SysResult};

use crate::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::{file::FanotifyGroupFile, inode::FanotifyGroupInode};

/// Dentry for an fanotify group file descriptor.
pub struct FanotifyGroupDentry {
    meta: DentryMeta,
}

impl FanotifyGroupDentry {
    /// Creates a new fanotify group dentry.
    pub fn new(inode: Option<Arc<FanotifyGroupInode>>) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new("fanotify", inode.map(|i| i as Arc<dyn Inode>), None),
        })
    }
}

impl Dentry for FanotifyGroupDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let file_meta = FileMeta::new(self.clone());
        *file_meta.internal_flags.lock() |= FileInternalFlags::FMODE_NONOTIFY;
        Ok(Arc::new(FanotifyGroupFile { meta: file_meta }))
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: InodeMode) -> SysResult<()> {
        Err(SysError::EACCES)
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::EACCES)
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        Err(SysError::EACCES)
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        Err(SysError::EACCES)
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        // fanotify group dentry is not a directory and cannot have children
        // TODO: maybe return an error instead?
        panic!("Cannot create new child for fanotify group dentry")
    }
}
