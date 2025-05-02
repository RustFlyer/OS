use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;

use config::vfs::SeekFrom;
use mutex::ShareMutex;
use systype::SysResult;
use vfs::file::{File, FileMeta};

use crate::{dentry::ExtDentry, ext::file::ExtFile, inode::file::ExtFileInode};

/// A [`File`] implementation for an ext4 regular file.
pub struct ExtRegFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

impl ExtRegFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for ExtRegFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let mut ext4_file = self.file.lock();
        if pos >= self.meta().dentry.inode().unwrap().size() {
            // `lwext4` will return an `EINVAL` error if the position is beyond the end
            // of the file when calling `seek`, which is not consistent with the `fseek`
            // function in `libc`. This is a workaround to fix it.
            return Ok(0);
        }
        ext4_file.seek(SeekFrom::Start(pos as u64))?;
        ext4_file.read(buf)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(pos as u64))?;
        file.write(buf)
    }
}
