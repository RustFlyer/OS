use alloc::{
    string::ToString,
    sync::{Arc, Weak},
};
use config::inode::{InodeMode, InodeType};
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
    path::Path,
    sys_root_dentry,
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
        log::debug!("[simple::base_create] set mode: {:?}", mode);
        inode.set_inotype(InodeType::from(mode));
        log::debug!(
            "[simple::base_create] set type: {:?}",
            InodeType::from(mode)
        );
        dentry.set_inode(inode);
        Ok(())
    }

    fn base_symlink(&self, dentry: &dyn Dentry, target: &str) -> SysResult<()> {
        let sb = self.superblock().ok_or(SysError::ENOTDIR)?;
        let inode = SimpleInode::new(sb);

        inode.set_inotype(InodeType::SymLink);
        inode.set_symlink_target(target);

        dentry.set_inode(inode);
        Ok(())
    }

    fn base_link(&self, dentry: &dyn Dentry, old_dentry: &dyn Dentry) -> SysResult<()> {
        let tinode = old_dentry.inode().ok_or(SysError::ENOENT)?;
        if tinode.inotype() == InodeType::Dir {
            return Err(SysError::EPERM);
        }

        let meta = tinode.get_meta();
        meta.inner.lock().nlink += 1;
        dentry.set_inode(tinode);

        Ok(())
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let name = dentry.name();
        log::debug!("[simple::base_lookup] name: {}", name);
        let _ = self.get_child(name).ok_or(SysError::ENOENT)?;

        Ok(())
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let this = self as Arc<dyn Dentry>;
        let dentry =
            Self::new(name, None, Some(Arc::downgrade(&(Arc::clone(&this))))) as Arc<dyn Dentry>;
        this.add_child(Arc::clone(&dentry));
        dentry as Arc<dyn Dentry>
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let dentry = self.clone().into_dyn();
        let inode = self.inode().ok_or(SysError::EEXIST)?;
        log::debug!("[simple::base_open] inode.inotype: {:?}", inode.inotype());

        match inode.inotype() {
            InodeType::Dir => Ok(SimpleDirFile::new(dentry.clone())),
            InodeType::File => Ok(SimpleFileFile::new(dentry.clone())),
            InodeType::SymLink => {
                log::debug!("[simple::base_open] open symlink: {}", dentry.name());
                Ok(SimpleFileFile::new(dentry))
            }
            _ => unimplemented!(),
        }
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        self.into_dyn_ref()
            .remove_child(dentry)
            .ok_or(SysError::ENOENT)
            .map(|_| ())
    }

    fn base_rmdir(&self, dentry: &dyn Dentry) -> SysResult<()> {
        if !dentry.get_meta().children.lock().is_empty() {
            return Err(SysError::ENOTEMPTY);
        }
        self.remove_child(dentry).unwrap();
        Ok(())
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
