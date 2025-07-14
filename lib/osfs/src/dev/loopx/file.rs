use super::{
    blkinfo::{BlkIoctlCmd, HdGeometry},
    loopinfo::{LoopInfo, LoopInfo64, LoopIoctlCmd},
};
use crate::dev::loopx::externf::__KernelTableIf_mod;
use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;
use config::{device::BLOCK_SIZE, vfs::SeekFrom};
use crate_interface::call_interface;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult, SyscallResult};
use vfs::{
    dentry::Dentry,
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct LoopFile {
    pub(crate) meta: FileMeta,
    pub(crate) inner: SpinNoIrqLock<LoopInfo64>,
    pub file: SpinNoIrqLock<Option<Arc<dyn File>>>,
}

impl LoopFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        let f = Self {
            meta: FileMeta::new(dentry),
            inner: SpinNoIrqLock::new(LoopInfo64::default()),
            file: SpinNoIrqLock::new(None),
        };
        Arc::new(f)
    }
}

#[async_trait]
impl File for LoopFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let lock = self.file.lock().clone();
        // log::error!("loop read");
        if let Some(file) = lock {
            // log::error!("loop read {} at offset {:#x}", file.dentry().path(), pos);
            file.seek(SeekFrom::Start(pos as u64))?;
            return file.read(buf).await;
        }
        Ok(0)
    }

    async fn base_write(&self, buf: &[u8], pos: usize) -> SysResult<usize> {
        let lock = self.file.lock().clone();
        if let Some(file) = lock {
            // log::error!("loop write {} at offset {:#x}", file.dentry().path(), pos);
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
        if let Some(cmd) = LoopIoctlCmd::from_repr(cmd as u32) {
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
                    log::error!("[loopx::ioctl] clear");
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
                        *(arg as *mut u32) = BLOCK_SIZE as u32;
                    }
                    Ok(0)
                }
                BlkIoctlCmd::BLKGETSIZE => {
                    let file = self.file.lock();
                    if let Some(ref f) = *file {
                        let size = f.inode().size();
                        unsafe {
                            *(arg as *mut u32) = (size / BLOCK_SIZE) as u32;
                        }
                        Ok(0)
                    } else {
                        Err(SysError::ENXIO)
                    }
                }
                BlkIoctlCmd::BLKFLSBUF => Ok(0),
                BlkIoctlCmd::FATIOCTLGETVOLUMEID => {
                    unsafe {
                        *(arg as *mut u32) = 0x12345678;
                    }
                    Ok(0)
                }
                BlkIoctlCmd::HDIOGETGEO => {
                    let old_geometry = HdGeometry {
                        heads: 255,      // 最大255个磁头
                        sectors: 63,     // 每磁道63个扇区
                        cylinders: 1024, // 1024个柱面
                        start: 0,        // 起始扇区为0
                    };
                    unsafe {
                        *(arg as *mut HdGeometry) = old_geometry;
                    }
                    Ok(0)
                }
            }
        } else {
            log::error!("[LoopFile::ioctl] cmd {cmd:#x} not included");
            unimplemented!()
        }
    }
}
