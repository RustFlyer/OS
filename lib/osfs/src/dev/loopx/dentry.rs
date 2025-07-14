use alloc::sync::{Arc, Weak};
use config::vfs::OpenFlags;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::Inode,
    path::Path,
    sys_root_dentry,
};

use super::{file::LoopFile, inode::LoopInode, loopinfo::LoopInfo64};

pub struct LoopDentry {
    meta: DentryMeta,
}

impl LoopDentry {
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

impl Dentry for LoopDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        // let file_meta = FileMeta::new(self);
        // *file_meta.flags.lock() = OpenFlags::O_RDWR;
        // Ok(Arc::new(LoopFile {
        //     meta: file_meta,
        //     inner: SpinNoIrqLock::new(LoopInfo64::default()),
        //     file: SpinNoIrqLock::new(None),
        // }))
        let inode = self.inode().ok_or(SysError::ENOENT)?;
        // log::error!("inode open {:#x}", inode.size());
        let loopinode = inode
            .downcast_arc::<LoopInode>()
            .unwrap_or_else(|_| unreachable!());
        Ok(loopinode.file.clone())
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: config::inode::InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let name = dentry.name();
        if let Some(minor) = name
            .strip_prefix("loop")
            .and_then(|n| n.parse::<u32>().ok())
        {
            let d = Path::new(sys_root_dentry(), dentry.path()).walk()?;
            let f = LoopFile::new(d);
            let inode = LoopInode::new(sys_root_dentry().superblock().unwrap(), minor, f);
            dentry.set_inode(inode);
            Ok(())
        } else {
            Err(SysError::ENOENT)
        }
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
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

    fn base_rmdir(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_rmdir_recur(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_symlink(&self, _dentry: &dyn Dentry, _target: &str) -> SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }
}
