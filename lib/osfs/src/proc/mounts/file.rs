use alloc::{boxed::Box, string::ToString};
use async_trait::async_trait;
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::FS_MANAGER;

pub struct MountsFile {
    pub(crate) meta: FileMeta,
}

#[async_trait]
impl File for MountsFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SyscallResult {
        let mut info = "".to_string();
        let fs_mgr = FS_MANAGER.lock();
        for (_fstype, fs) in fs_mgr.iter() {
            let supers = fs.get_meta().sblks.lock();
            for (_mount_path, _sb) in supers.iter() {
                // info += "proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n"
            }
        }
        let len = info.len();
        if self.pos() >= len {
            return Ok(0);
        }
        buf[..len].copy_from_slice(info.as_bytes());
        Ok(len)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }
}
