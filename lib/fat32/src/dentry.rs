use alloc::sync::{Arc, Weak};

use config::inode::{InodeMode, InodeType};
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use crate::{
    as_sys_err,
    file::{dir::FatDirFile, file::FatFileFile},
    inode::{dir::FatDirInode, file::FatFileInode},
};

pub struct FatDentry {
    meta: DentryMeta,
}

impl FatDentry {
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

impl Dentry for FatDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()> {
        let name = dentry.name();
        log::trace!("[FatDentry::base_create] create name {name}, mode {mode:?}");
        let sb = self.superblock().ok_or(SysError::ENOENT)?;
        let inode = self
            .inode()
            .ok_or(SysError::ENOENT)?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;

        let sub_dentry = dentry;
        match mode.to_type() {
            InodeType::Dir => {
                let new_dir = inode.dir.lock().create_dir(name).map_err(as_sys_err)?;
                let new_inode = FatDirInode::new(sb.clone(), new_dir);
                sub_dentry.set_inode(new_inode);
                Ok(())
            }
            InodeType::File => {
                let new_file = inode.dir.lock().create_file(name).map_err(as_sys_err)?;
                let new_inode = FatFileInode::new(sb.clone(), new_file);
                sub_dentry.set_inode(new_inode);
                Ok(())
            }
            _ => {
                log::warn!("[FatDentry::base_create] not supported mode {mode:?}");
                Err(SysError::EIO)
            }
        }
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let sb = self.superblock().ok_or(SysError::ENOENT)?;
        let name = dentry.name();
        let inode = self
            .inode()
            .ok_or(SysError::ENOENT)?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;

        let find = inode.dir.lock().iter().find(|e| {
            let entry = e.as_ref().unwrap();
            let e_name = entry.file_name();
            name == e_name
        });

        let sub_dentry = dentry;
        if let Some(find) = find {
            log::debug!("[FatDentry::base_lookup] find name {name}");
            let entry = find.map_err(as_sys_err)?;
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(sb, new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(sb, new_file)
            };
            sub_dentry.set_inode(new_inode);
        } else {
            log::warn!("[FatDentry::base_lookup] name {name} does not exist");
        }
        Ok(())
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.inode(), Some(Arc::downgrade(&self.into_dyn())))
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self.inode().ok_or(SysError::ENOENT)?;
        match inode.inotype() {
            InodeType::File => {
                let inode = inode
                    .downcast_arc::<FatFileInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatFileFile::new(self.clone(), inode))
            }
            InodeType::Dir => {
                let inode = inode
                    .downcast_arc::<FatDirInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatDirFile::new(self.clone(), inode))
            }
            _ => Err(SysError::EPERM),
        }
    }

    fn base_rename(
        &self,
        dentry: &dyn Dentry,
        new_dir: &dyn Dentry,
        new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        log::debug!(
            "[FatDentry::base_rename] rename {} to {}",
            dentry.path(),
            new_dentry.path()
        );

        let dir_inode = self
            .inode()
            .ok_or(SysError::ENOENT)?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let dir = dir_inode.dir.lock();

        let new_dir_inode = new_dir
            .inode()
            .ok_or(SysError::ENOENT)?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let new_dir = new_dir_inode.dir.lock();

        dir.rename(dentry.name(), &new_dir, new_dentry.name())
            .map_err(as_sys_err)?;

        new_dentry.set_inode(dentry.inode().ok_or(SysError::ENOENT)?);
        dentry.unset_inode();
        dentry.get_meta().children.lock().clear();
        Ok(())
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let inode = self
            .inode()
            .ok_or(SysError::ENOENT)?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        inode.dir.lock().remove(dentry.name()).map_err(as_sys_err)?;
        Ok(())
    }
}
