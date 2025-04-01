use alloc::{ffi::CString, sync::Arc};
use config::{inode::InodeType, vfs::OpenFlags};
use lwext4_rust::{
    InodeTypes,
    bindings::{EOK, ext4_dir_rm, ext4_flink, ext4_fremove, ext4_inode_exist},
};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
    superblock::SuperBlock,
};

use systype::{SysResult, SyscallResult};

use crate::{
    ext::{dir::ExtDir, file::ExtFile},
    file::{dir::ExtDirFile, file::ExtFileFile},
    inode::{dir::ExtDirInode, file::ExtFileInode, link::ExtLinkInode},
};

pub struct ExtDentry {
    meta: DentryMeta,
}

impl ExtDentry {
    pub fn new(
        name: &str,
        superblock: Arc<dyn SuperBlock>,
        parentdentry: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, superblock, parentdentry),
        })
    }

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }
}

impl Dentry for ExtDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    /// When Dentry acts as a Dir, it can create a sub-dentry with a specific mode
    /// - InodeType::File -> ExtFileInode
    /// - InodeType::Dir  -> ExtDirInode
    ///
    /// Returns a result of sub dentry
    fn base_create(
        self: Arc<Self>,
        name: &str,
        mode: config::inode::InodeMode,
    ) -> SysResult<Arc<dyn Dentry>> {
        let superblock = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<ExtDirInode>()
            .expect("Only Dir can Create!");

        let sub_dentry = self.into_dyn().get_child_or_create(name);
        let path = sub_dentry.path();
        let new_inode: Arc<dyn Inode> = match mode.to_type() {
            InodeType::File => {
                let flags = (OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC).bits();
                let file = ExtFile::open(&path, flags)?;
                ExtFileInode::new(superblock, file)
            }
            InodeType::Dir => {
                let dir = ExtDir::create(&path)?;
                ExtDirInode::new(superblock, dir)
            }
            _ => todo!(),
        };
        sub_dentry.set_inode(new_inode);
        Ok(sub_dentry)
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let superblock = self.super_block();
        let sub_dentry = self.into_dyn().get_child(name)?;
        let path = sub_dentry.path();
        let c_path = CString::new(path).expect("CString::new failed");
        if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_DIR as i32) } == EOK {
            let new_file = ExtDir::open(&path)?;
            sub_dentry.set_inode(ExtDirInode::new(superblock, new_file))
        } else if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_REG_FILE as i32) }
            == EOK
        {
            let new_file = ExtFile::open(&path, OpenFlags::empty().bits())?;
            sub_dentry.set_inode(ExtFileInode::new(superblock, new_file))
        } else if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_SYMLINK as i32) }
            == EOK
        {
            let path = sub_dentry.path();
            let mut path_buf = vec![0; 512];
            let c_path = CString::new(path).expect("CString::new failed");
            let mut r_cnt = 0;
            let len = unsafe {
                ext4_readlink(
                    c_path.as_ptr(),
                    buf.as_mut_ptr() as _,
                    buf.len(),
                    &mut r_cnt,
                )
            }?;
            path_buf.truncate(len + 1);
            let target = CString::from_vec_with_nul(path_buf)?;
            let sub_inode = ExtLinkInode::new(target.to_str().unwrap(), superblock);
            sub_dentry.set_inode(sub_inode)
        }
        Ok(sub_dentry)
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        match self.inode()?.inotype() {
            InodeType::File => {
                let inode = self.inode()?.downcast_arc::<ExtFileInode>()?;
                Ok(ExtFileFile::new(self, inode))
            }
            InodeType::Dir => {
                let inode = self.inode()?.downcast_arc::<ExtDirInode>()?;
                Ok(ExtDirFile::new(self, inode))
            }
            InodeType::SymLink => {
                let inode = self.inode()?.downcast_arc::<ExtLinkInode>()?;
                Ok(ExtLinkInode::new(self, inode))
            }
            _ => todo!(),
        }
    }

    fn base_link(self: Arc<Self>, new: &Arc<dyn Dentry>) -> SysResult<()> {
        let sblk = self.super_block();
        let oldpath = self.path();
        let newpath = new.path();

        unsafe {
            ext4_flink(oldpath, newpath);
        }
        new.set_inode(self.inode()?);
        Ok(())
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult {
        let sub_dentry = self.get_child(name)?;
        let path = sub_dentry.path();
        match sub_dentry.inode()?.inotype() {
            InodeType::Dir => unsafe { ext4_dir_rm(path) },
            InodeType::File | InodeType::SymLink => unsafe { ext4_fremove(path) },
            _ => todo!(),
        }
    }

    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult {}
}
