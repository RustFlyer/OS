extern crate alloc;
use alloc::{ffi::CString, sync::Arc};

use lwext4_rust::bindings::ext4_readlink;
use systype::SysResult;
use vfs::file::{File, FileMeta};

use crate::{dentry::ExtDentry, inode::link::ExtLinkInode};
pub struct ExtLinkFile {
    meta: FileMeta,
}

unsafe impl Send for ExtLinkFile {}
unsafe impl Sync for ExtLinkFile {}

impl ExtLinkFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtLinkInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
        })
    }
}

impl File for ExtLinkFile {
    fn get_meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read_link(&self, buf: &mut [u8]) -> SysResult<usize> {
        let path = self.dentry().path();
        let path_buf = buf;
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
        Ok(len)
    }
}
