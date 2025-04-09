use alloc::{
    ffi::CString,
    sync::{Arc, Weak},
    vec,
};

use lwext4_rust::{
    InodeTypes,
    bindings::{ext4_dir_rm, ext4_flink, ext4_fremove, ext4_inode_exist, ext4_readlink},
};

use config::{
    inode::{InodeMode, InodeType},
    vfs::OpenFlags,
};
use systype::{SysError, SysResult, SyscallResult};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use crate::{
    ext::{dir::ExtDir, file::ExtFile},
    file::{dir::ExtDirFile, file::ExtFileFile, link::ExtLinkFile},
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
        let path = dentry.path();
        let superblock = self.superblock().unwrap();
        let new_inode: Arc<dyn Inode> = match mode.to_type() {
            InodeType::Dir => {
                let new_dir = ExtDir::create(&path).map_err(SysError::from_i32)?;
                let inode = ExtDirInode::new(superblock, new_dir);
                inode.set_inotype(InodeType::Dir);
                inode
            }
            InodeType::File => {
                let new_file = ExtFile::open(
                    &path,
                    (OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC).bits(),
                )
                .map_err(SysError::from_i32)?;
                let inode = ExtFileInode::new(superblock, new_file);
                inode.set_inotype(InodeType::File);
                inode
            }
            _ => unimplemented!("Unsupported file type"),
        };
        dentry.set_inode(new_inode);
        Ok(())
    }

    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let superblock = self.superblock().unwrap();
        let path = dentry.path();
        let c_path = CString::new(path.clone()).unwrap();
        if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_DIR as i32) == 0 } {
            let new_file = ExtDir::open(&path).map_err(SysError::from_i32)?;
            let inode = ExtDirInode::new(superblock, new_file);
            inode.set_inotype(InodeType::Dir);
            dentry.set_inode(inode);
            Ok(())
        } else if unsafe {
            ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_REG_FILE as i32) == 0
        } {
            let new_file =
                ExtFile::open(&path, OpenFlags::empty().bits()).map_err(SysError::from_i32)?;
            let inode = ExtFileInode::new(superblock, new_file);
            inode.set_inotype(InodeType::File);
            dentry.set_inode(inode);
            Ok(())
        } else if unsafe {
            ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_SYMLINK as i32) == 0
        } {
            let mut target = vec![0; 512];
            let mut bytes_read = 0;
            unsafe {
                let err = ext4_readlink(
                    c_path.as_ptr(),
                    target.as_mut_ptr(),
                    target.len() - 1,
                    &mut bytes_read,
                );
                if err != 0 {
                    return Err(SysError::from_i32(err));
                }
            };
            target.truncate(bytes_read + 1);
            let target = unsafe { CString::from_vec_with_nul_unchecked(target) };
            let inode = ExtLinkInode::new(target.to_str().unwrap(), superblock);
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
        log::debug!("{:?}", inode.inotype());
        match inode.inotype() {
            InodeType::File => {
                let inode = inode
                    .downcast_arc::<ExtFileInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtFileFile::new(self, inode))
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
        let oldpath = old_dentry.path();
        let newpath = dentry.path();
        let c_oldpath = CString::new(oldpath).unwrap();
        let c_newpath = CString::new(newpath).unwrap();

        unsafe {
            ext4_flink(c_oldpath.as_ptr(), c_newpath.as_ptr());
        }
        dentry.set_inode(self.inode().unwrap());
        Ok(())
    }

    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let path = dentry.path();
        let c_path = CString::new(path).unwrap();
        let err = unsafe { ext4_fremove(c_path.as_ptr()) };
        if err != 0 {
            return Err(SysError::from_i32(err));
        }
        Ok(())
    }

    fn base_rmdir(&self, dentry: &dyn Dentry) -> SysResult<()> {
        let path = dentry.path();
        let c_path = CString::new(path).unwrap();
        let err = unsafe { ext4_dir_rm(c_path.as_ptr()) };
        if err != 0 {
            return Err(SysError::from_i32(err));
        }
        Ok(())
    }
}
