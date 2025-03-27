extern crate alloc;
use alloc::sync::Arc;

use config::inode::InodeType;
use lwext4_rust::bindings::SEEK_SET;
use mutex::ShareMutex;
use systype::{SysError, SyscallResult};
use vfs::{
    dentry,
    file::{File, FileMeta},
};

use crate::{dentry::ExtDentry, ext::file::ExtFile, inode::file::ExtFileInode};

pub struct ExtFileFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileFile {}
unsafe impl Sync for ExtFileFile {}

impl ExtFileFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for ExtFileFile {
    fn get_meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                file.seek(offset, SEEK_SET)?;
                file.read(buf)?;
            }
            _ => todo!(),
        }
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                file.seek(offset, SEEK_SET)?;
                file.write(buf)?;
            }
            _ => todo!(),
        }
    }
}
