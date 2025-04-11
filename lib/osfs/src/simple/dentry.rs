use alloc::sync::{Arc, Weak};
use config::inode::{InodeMode, InodeType};
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::{file::SimpleDirFile, inode::SimpleInode};

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

    pub fn into_dyn_ref(&self) -> &dyn Dentry {
        self
    }
}

impl Dentry for SimpleDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()> {
        let sb = self.superblock().ok_or(SysError::ENOTDIR)?;
        let inode = SimpleInode::new(sb);
        inode.set_inotype(InodeType::from(mode));
        dentry.set_inode(inode);
        Ok(())
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let name = dentry.name();
        let child = self.get_child(name).ok_or(SysError::ENOENT)?;
        let sb = self.superblock().ok_or(SysError::ENOTDIR)?;
        let inode = SimpleInode::new(sb);
        inode.set_inotype(InodeType::File);
        child.set_inode(inode);
        Ok(())
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.inode(), Some(Arc::downgrade(&self.into_dyn())))
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
