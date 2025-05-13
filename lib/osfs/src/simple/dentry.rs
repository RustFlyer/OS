use alloc::sync::{Arc, Weak};
use config::inode::{InodeMode, InodeType};
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use super::{
    file::{SimpleDirFile, SimpleFileFile},
    inode::SimpleInode,
};

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
        log::debug!("[simple::base_lookup] name: {}", name);
        let child = self.get_child(name).ok_or(SysError::ENOENT)?;
        let sb = self.superblock().ok_or(SysError::ENOTDIR)?;
        let inode = SimpleInode::new(sb);
        inode.set_inotype(InodeType::File);
        child.set_inode(inode);

        log::debug!(
            "[simple::base_lookup] inotype: {:?}",
            child.inode().unwrap().inotype()
        );

        Ok(())
    }

    // fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
    //     Self::new(name, self.inode(), Some(Arc::downgrade(&self.into_dyn())))
    // }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let this = self as Arc<dyn Dentry>;
        let dentry =
            Self::new(name, None, Some(Arc::downgrade(&(Arc::clone(&this))))) as Arc<dyn Dentry>;
        this.add_child(Arc::clone(&dentry));
        dentry as Arc<dyn Dentry>
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self.inode().ok_or(SysError::EEXIST)?;
        log::debug!("[simple::base_open] inode.inotype: {:?}", inode.inotype());
        match inode.inotype() {
            InodeType::Dir => Ok(SimpleDirFile::new(self.clone())),
            InodeType::File => Ok(SimpleFileFile::new(self.clone())),
            _ => unreachable!(),
        }
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        self.into_dyn_ref()
            .remove_child(dentry)
            .ok_or(SysError::ENOENT)
            .map(|_| ())
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
