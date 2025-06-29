use alloc::{boxed::Box, ffi::CString, sync::Arc};
use core::panic;

use async_trait::async_trait;
use systype::error::SysResult;
use vfs::file::{File, FileMeta};

use crate::{dentry::ExtDentry, ext::file::ExtFile, inode::link::ExtLinkInode};
pub struct ExtLinkFile {
    meta: FileMeta,
}

unsafe impl Send for ExtLinkFile {}
unsafe impl Sync for ExtLinkFile {}

impl ExtLinkFile {
    pub fn new(dentry: Arc<ExtDentry>, _inode: Arc<ExtLinkInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

#[async_trait]
impl File for ExtLinkFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        panic!("`base_read` is not supported for this file type");
    }

    async fn base_write(&self, _buf: &[u8], _pos: usize) -> SysResult<usize> {
        panic!("`base_write` is not supported for this file type");
    }

    fn base_readlink(&self, buf: &mut [u8]) -> SysResult<usize> {
        // let mut ext4_file = self.file.lock();
        // let file_size = ext4_file.size() as usize;
        let this_path = self.meta().dentry.path();
        let this_path = CString::new(this_path).unwrap();
        ExtFile::readlink(&this_path, buf)
    }
}
