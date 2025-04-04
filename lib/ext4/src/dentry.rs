use alloc::{
    ffi::CString,
    sync::{Arc, Weak},
    vec,
};
use config::{
    inode::{InodeMode, InodeType},
    vfs::OpenFlags,
};

use lwext4_rust::{
    InodeTypes,
    bindings::{EOK, ext4_dir_rm, ext4_flink, ext4_fremove, ext4_inode_exist, ext4_readlink},
};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    inode::Inode,
};

use systype::{SysError, SysResult, SyscallResult};

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
        let inode = self
            .inode()
            .unwrap()
            .downcast_arc::<ExtDirInode>()
            .unwrap_or_else(|_| unreachable!());
        let mut dir = inode.dir.lock();
        let new_inode: Arc<dyn Inode> = match mode.to_type() {
            InodeType::Dir => {
                let new_dir = LwExt4Dir::create(&path).map_err(SysError::from_i32)?;
                ExtDirInode::new(sb, new_dir)
            }
            InodeType::File => {
                let new_file = LwExt4File::open(
                    &path,
                    (OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC).bits(),
                )
                .map_err(SysError::from_i32)?;
                Ext4FileInode::new(sb, new_file)
            }
            _ => todo!(),
        };
        sub_dentry.set_inode(new_inode);
        Ok(sub_dentry)
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let superblock = self.super_block();
        let sub_dentry = self.into_dyn().get_child(name).unwrap();
        let path = sub_dentry.path();
        let c_path = CString::new(path.clone()).expect("CString::new failed");
        if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_DIR as i32) }
            == EOK as i32
        {
            let new_file = ExtDir::open(&path).map_err(SysError::from_i32)?;
            sub_dentry.set_inode(ExtDirInode::new(superblock, new_file))
        } else if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_REG_FILE as i32) }
            == EOK as i32
        {
            let new_file =
                ExtFile::open(&path, OpenFlags::empty().bits()).map_err(SysError::from_i32)?;
            sub_dentry.set_inode(ExtFileInode::new(superblock, new_file))
        } else if unsafe { ext4_inode_exist(c_path.as_ptr(), InodeTypes::EXT4_DE_SYMLINK as i32) }
            == EOK as i32
        {
            let path = sub_dentry.path();
            let mut path_buf = vec![0; 512];
            let c_path = CString::new(path).expect("CString::new failed");
            let mut r_cnt = 0;
            let len = unsafe {
                ext4_readlink(
                    c_path.as_ptr(),
                    path_buf.as_mut_ptr() as _,
                    path_buf.len(),
                    &mut r_cnt,
                ) as usize
            };
            path_buf.truncate(len + 1);
            let target = CString::from_vec_with_nul(path_buf).unwrap();
            let sub_inode = ExtLinkInode::new(target.to_str().unwrap(), superblock);
            sub_dentry.set_inode(sub_inode)
        }
        Ok(sub_dentry)
    }

    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        match self.inode()?.inotype() {
            InodeType::File => {
                let inode = self
                    .inode()?
                    .downcast_arc::<ExtFileInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtFileFile::new(self, inode))
            }
            InodeType::Dir => {
                let inode = self
                    .inode()?
                    .downcast_arc::<ExtDirInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtDirFile::new(self, inode))
            }
            InodeType::SymLink => {
                let inode = self
                    .inode()?
                    .downcast_arc::<ExtLinkInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(ExtLinkFile::new(self, inode))
            }
            _ => todo!(),
        }
    }

    fn base_link(self: Arc<Self>, new: &Arc<dyn Dentry>) -> SysResult<()> {
        let oldpath = self.path();
        let newpath = new.path();
        let c_oldpath = CString::new(oldpath).expect("CString::new failed");
        let c_newpath = CString::new(newpath).expect("CString::new failed");

        unsafe {
            ext4_flink(c_oldpath.as_ptr(), c_newpath.as_ptr());
        }
        new.set_inode(self.inode()?);
        Ok(())
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult {
        let sub_dentry = self.get_child(name).unwrap();
        let path = sub_dentry.path();
        let c_path = CString::new(path).expect("CString::new failed");
        let ret = match sub_dentry.inode()?.inotype() {
            InodeType::Dir => unsafe { ext4_dir_rm(c_path.as_ptr()) },
            InodeType::File | InodeType::SymLink => unsafe { ext4_fremove(c_path.as_ptr()) },
            _ => todo!(),
        };
        Ok(ret as usize)
    }

    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult {
        todo!()
    }
}
