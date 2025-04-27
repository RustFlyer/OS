use alloc::{
    ffi::CString,
    sync::{Arc, Weak},
};

use lwext4_rust::InodeTypes;

use config::{
    inode::{InodeMode, InodeType},
    vfs::OpenFlags,
};
use systype::{SysError, SysResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use crate::{
    ext::{dir::ExtDir, file::ExtFile, inode::ExtInode},
    file::{dir::ExtDirFile, link::ExtLinkFile, reg::ExtRegFile},
    inode::{dir::ExtDirInode, file::ExtFileInode, link::ExtLinkInode},
};

pub struct ExtDentry {
    meta: DentryMeta,
}

impl ExtDentry {
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

impl Dentry for ExtDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()> {
        let path = CString::new(dentry.path()).unwrap();
        let superblock = self.superblock().unwrap();
        let new_inode: Arc<dyn Inode> = match mode.to_type() {
            InodeType::Dir => {
                ExtDir::create(&path)?;
                let new_dir = ExtDir::open(&path)?;
                let inode = ExtDirInode::new(superblock, new_dir);
                inode.set_inotype(InodeType::Dir);
                inode
            }
            InodeType::File => {
                let new_file = ExtFile::open2(
                    &path,
                    OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
                )?;
                let inode = ExtFileInode::new(superblock, new_file);
                inode.set_inotype(InodeType::File);
                inode
            }
            _ => unimplemented!("Unsupported file type"),
        };
        dentry.set_inode(new_inode);
        // log::error!("[base_create] {} set inode", dentry.path());
        Ok(())
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let superblock = self.superblock().unwrap();
        let path = CString::new(dentry.path()).unwrap();
        if ExtInode::exists(&path, InodeTypes::EXT4_DE_DIR)? {
            let new_file = ExtDir::open(&path)?;
            let inode = ExtDirInode::new(superblock, new_file);
            inode.set_inotype(InodeType::Dir);
            dentry.set_inode(inode);
            Ok(())
        } else if ExtInode::exists(&path, InodeTypes::EXT4_DE_REG_FILE)? {
            let new_file = ExtFile::open2(&path, OpenFlags::empty())?;
            let inode = ExtFileInode::new(superblock, new_file);
            inode.set_inotype(InodeType::File);
            dentry.set_inode(inode);
            Ok(())
        } else if ExtInode::exists(&path, InodeTypes::EXT4_DE_SYMLINK)? {
            let new_file = ExtFile::open2(&path, OpenFlags::empty())?;
            let inode = ExtLinkInode::new(superblock, new_file);
            inode.set_inotype(InodeType::SymLink);
            dentry.set_inode(inode);
            Ok(())
        } else {
            Err(SysError::ENOENT)
        }
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let this = self as Arc<dyn Dentry>;
        let dentry = ExtDentry::new(name, None, Some(Arc::downgrade(&(Arc::clone(&this)))))
            as Arc<dyn Dentry>;
        this.add_child(Arc::clone(&dentry));
        dentry as Arc<dyn Dentry>
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self.inode().unwrap();
        match inode.inotype() {
            InodeType::File => {
                let inode = inode
                    .downcast_arc::<ExtFileInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtRegFile::new(self, inode))
            }
            InodeType::Dir => {
                let inode = inode
                    .downcast_arc::<ExtDirInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtDirFile::new(self, inode))
            }
            InodeType::SymLink => {
                let inode = inode
                    .downcast_arc::<ExtLinkInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtLinkFile::new(self, inode))
            }
            _ => unimplemented!("Unsupported file type"),
        }
    }

    fn base_link(&self, dentry: &dyn Dentry, old_dentry: &dyn Dentry) -> SysResult<()> {
        let old_path = CString::new(old_dentry.path()).unwrap();
        let new_path = CString::new(dentry.path()).unwrap();
        ExtFile::link(&old_path, &new_path)?;
        dentry.set_inode(self.inode().unwrap());
        Ok(())
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        self.meta.children.lock().remove(dentry.name());
        let path = CString::new(dentry.path()).unwrap();
        ExtFile::unlink(&path)?;
        dentry.unset_inode();
        Ok(())
    }

    fn base_rmdir(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let path = CString::new(dentry.path()).unwrap();
        ExtDir::remove_recursively(&path)?;
        dentry.unset_inode();
        Ok(())
    }

    fn base_rename(
        &self,
        dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        let old_path = CString::new(dentry.path()).unwrap();
        let new_path = CString::new(new_dentry.path()).unwrap();
        ExtFile::rename(&old_path, &new_path)?;

        new_dentry.set_inode(dentry.inode().unwrap());
        dentry.unset_inode();
        dentry.get_meta().children.lock().clear();
        Ok(())
    }
}
