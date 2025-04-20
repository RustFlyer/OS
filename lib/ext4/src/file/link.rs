use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;

use config::vfs::SeekFrom;
use mutex::ShareMutex;
use systype::{SysError, SysResult};
use vfs::file::{File, FileMeta};

use crate::{dentry::ExtDentry, ext::file::ExtFile, inode::link::ExtLinkInode};
pub struct ExtLinkFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtLinkFile {}
unsafe impl Sync for ExtLinkFile {}

impl ExtLinkFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtLinkInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for ExtLinkFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let mut ext4_file = self.file.lock();
        ext4_file.seek(SeekFrom::Start(pos as u64))?;
        ext4_file.read(buf)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(pos as u64))?;
        file.write(buf)
    }

    fn base_readlink(&self, mut buf: &mut [u8]) -> SysResult<usize> {
        let mut ext4_file = self.file.lock();
        let file_size = ext4_file.size() as usize;
        if buf.len() < file_size {
            return Err(SysError::EINVAL);
        }
        buf = &mut buf[..file_size];
        ext4_file.seek(SeekFrom::Start(0))?;
        ext4_file.read(buf)
    }
}
