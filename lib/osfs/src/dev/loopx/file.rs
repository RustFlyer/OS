use super::{
    blkinfo::BlkIoctlCmd,
    loopinfo::{LoopInfo64, LoopIoctlCmd},
};
use crate::dev::loopx::externf::__KernelTableIf_mod;
use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;
use config::vfs::SeekFrom;
use crate_interface::call_interface;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct LoopFile {
    pub(crate) meta: FileMeta,
    pub(crate) inner: SpinNoIrqLock<LoopInfo64>,
    pub file: SpinNoIrqLock<Option<Arc<dyn File>>>,
}

#[async_trait]
impl File for LoopFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let lock = self.file.lock().clone();
        log::error!("loop read");
        if let Some(file) = lock {
            log::error!("loop read {} at offset {}", file.dentry().path(), pos);
            file.seek(SeekFrom::Start(pos as u64))?;
            return file.read(buf).await;
        }
        Ok(0)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        let lock = self.file.lock().clone();
        log::error!("loop write");
        if let Some(file) = lock {
            log::error!("loop write {} at offset {}", file.dentry().path(), pos);
            file.seek(SeekFrom::Start(pos as u64))?;
            return file.write(buf).await;
        }
        Ok(0)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        if let Some(cmd) = LoopIoctlCmd::from_repr(cmd) {
            match cmd {
                LoopIoctlCmd::SETFD => {
                    let table = call_interface!(KernelTableIf::table());
                    let file = table.lock().get_file(arg)?;
                    let path = file.dentry().path();
                    *self.file.lock() = Some(file);

                    let mut info = LoopInfo64::default();

                    let bytes = path.as_bytes();
                    let len = bytes.len().min(63);
                    info.lo_file_name[..len].copy_from_slice(&bytes[..len]);
                    info.lo_file_name[len] = 0;

                    *self.inner.lock() = info;
                    Ok(0)
                }
                LoopIoctlCmd::CLRFD => {
                    *self.file.lock() = None;
                    Ok(0)
                }
                LoopIoctlCmd::SETSTATUS => {
                    let file = self.file.lock();
                    if file.is_none() {
                        return Err(SysError::ENXIO);
                    }

                    unsafe {
                        let info = *(arg as *const LoopInfo64);
                        *self.inner.lock() = info;
                    }

                    Ok(0)
                }
                LoopIoctlCmd::GETSTATUS => {
                    let file = self.file.lock();
                    if file.is_none() {
                        return Err(SysError::ENXIO);
                    }

                    let info = *self.inner.lock();
                    log::debug!("loopinfo: {:?}", info);
                    unsafe {
                        *(arg as *mut LoopInfo64) = info;
                    }
                    Ok(0)
                }
                LoopIoctlCmd::GETSTATUS64 => {
                    let info = *self.inner.lock();
                    unsafe {
                        *(arg as *mut LoopInfo64) = info;
                    }
                    Ok(0)
                }
                e => {
                    log::error!("not implement {:?}", e);
                    Err(SysError::ENOTTY)
                }
            }
        } else if let Some(cmd) = BlkIoctlCmd::from_repr(cmd) {
            match cmd {
                BlkIoctlCmd::BLKGETSIZE64 => {
                    let file = self.file.lock();
                    if let Some(ref f) = *file {
                        let size = f.inode().size() as u64;
                        unsafe {
                            *(arg as *mut u64) = size;
                        }
                        Ok(0)
                    } else {
                        Err(SysError::ENXIO)
                    }
                }
                BlkIoctlCmd::BLKSSZGET => {
                    unsafe {
                        *(arg as *mut u32) = 512;
                    }
                    Ok(0)
                }
                BlkIoctlCmd::BLKGETSIZE => {
                    let file = self.file.lock();
                    if let Some(ref f) = *file {
                        let size = f.inode().size() as u64;
                        unsafe {
                            *(arg as *mut u32) = (size / 512) as u32;
                        }
                        Ok(0)
                    } else {
                        Err(SysError::ENXIO)
                    }
                }
                BlkIoctlCmd::BLKFLSBUF => Ok(0),
            }
        } else {
            log::error!("[LoopFile::ioctl] cmd {cmd:#x} not included");
            unimplemented!()
        }
    }
}
