use alloc::sync::Arc;

use mutex::ShareMutex;
use systype::{SysError, SyscallResult};
use vfs::file::{File, FileMeta};

use crate::{
    dentry::ExtDentry,
    ext::file::{ExtFile, FileSeekType},
    inode::file::ExtFileInode,
};

pub struct ExtFileFile {
    meta: FileMeta,
    file: ShareMutex<ExtFile>,
}

unsafe impl Send for ExtFileFile {}
unsafe impl Sync for ExtFileFile {}

impl ExtFileFile {
    pub fn new(dentry: Arc<ExtDentry>, inode: Arc<ExtFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

impl File for ExtFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read(&self, buf: &mut [u8], pos: usize) -> SyscallResult {
        let mut file = self.file.lock();
        file.seek(pos as i64, FileSeekType::SeekSet)
            .map_err(SysError::from_i32)?;
        file.read(buf).map_err(SysError::from_i32)
    }

    fn base_write(&self, buf: &[u8], pos: usize) -> SyscallResult {
        let mut file = self.file.lock();
        file.seek(pos as i64, FileSeekType::SeekSet)
            .map_err(SysError::from_i32)?;
        file.write(buf).map_err(SysError::from_i32)
    }
}
