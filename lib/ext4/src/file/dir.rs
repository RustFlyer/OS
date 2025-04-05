extern crate alloc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::{ffi::CString, vec};
use config::vfs::OpenFlags;
use lwext4_rust::{InodeTypes, bindings::ext4_readlink};
use mutex::ShareMutex;
use systype::SysError;
use vfs::{
    direntry,
    file::{File, FileMeta},
    inode::Inode,
};

use crate::inode::link::ExtLinkInode;
use crate::{
    dentry::ExtDentry,
    ext::{dir::ExtDir, file::ExtFile},
    inode::{dir::ExtDirInode, file::ExtFileInode},
};

pub struct ExtDirFile {
    meta: FileMeta,
    dir: ShareMutex<ExtDir>,
}

unsafe impl Send for ExtDirFile {}
unsafe impl Sync for ExtDirFile {}

impl ExtDirFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            dir: inode.dir.clone(),
        })
    }
}

impl File for ExtDirFile {
    fn get_meta(&self) -> &FileMeta {
        &self.meta
    }

    /// # Here We should implement a function to load all dentry and inodes in a directory.
    fn base_load_dir(&self) -> systype::SysResult<()> {
        let mut dir = self.dir.lock();

        dir.next();
        dir.next();

        while let Some(dentry) = dir.next() {
            let name = CString::new(dentry.name).expect("cstring convert fail");
            let sub_dentry = self.dentry().get_child_or_create(name.to_str().unwrap());
            let new_inode: Arc<dyn Inode> =
                if InodeTypes::from(dentry.type_ as usize) == InodeTypes::EXT4_DE_REG_FILE {
                    let ext_file = ExtFile::open(&sub_dentry.path(), OpenFlags::O_RDWR.bits())
                        .map_err(SysError::from_i32)?;
                    ExtFileInode::new(self.super_block(), ext_file).clone()
                } else if InodeTypes::from(dentry.type_ as usize) == InodeTypes::EXT4_DE_DIR {
                    let ext_dir = ExtDir::open(&sub_dentry.path()).map_err(SysError::from_i32)?;
                    ExtDirInode::new(self.super_block(), ext_dir).clone()
                } else {
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
                        )
                    } as usize;
                    path_buf.truncate(len + 1);
                    let target =
                        CString::from_vec_with_nul(path_buf).expect("cstring vec convert fail");
                    ExtLinkInode::new(target.to_str().unwrap(), self.super_block()).clone()
                };
            if sub_dentry.is_negetive() {
                sub_dentry.set_inode(new_inode);
            }
        }

        Ok(())
    }

    fn base_ls(&self, path: String) {
        self.dir.lock().lwext4_dir_entries(&path);
    }
}
