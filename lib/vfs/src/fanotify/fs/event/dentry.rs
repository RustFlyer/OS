use alloc::sync::{Arc, Weak};

use config::inode::InodeMode;
use systype::error::{SysError, SysResult};

use crate::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
};

use super::{file::FanotifyEventFile, inode::FanotifyEventInode};

/// Dentry for a fanotify event file descriptor.
///
/// This represents the dentry that corresponds to the fanotify event file.
/// It's not part of the normal dentry tree but exists as a special file descriptor.
pub struct FanotifyEventDentry {
    meta: DentryMeta,
}

impl FanotifyEventDentry {
    /// Creates a new fanotify event dentry.
    ///
    /// # Arguments
    /// * `inode` - The fanotify event inode
    /// * `parent` - Parent dentry (typically None for a special file descriptor)
    pub fn new(
        inode: Option<Arc<FanotifyEventInode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new("fanotify-event", inode.map(|i| i as Arc<dyn Inode>), parent),
        })
    }
}

impl Dentry for FanotifyEventDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(FanotifyEventFile {
            meta: FileMeta::new(self),
        }))
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
        // fanotify event dentry is not a directory and cannot have children
        // TODO: maybe return an error instead?
        panic!("Cannot create new child for fanotify event dentry")
    }
}
