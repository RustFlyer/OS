use alloc::vec::Vec;
use alloc::{string::ToString, sync::Arc};
use core::task::Waker;
use spin::Mutex;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::{
    event::{FsConfigCommand, FsContext},
    flags::FsopenFlags,
};

pub struct FsContextInode {
    meta: InodeMeta,
    flags: FsopenFlags,
    /// The filesystem context
    context: Mutex<FsContext>,
    /// Waker queue for blocked readers
    wakers: Mutex<Vec<Waker>>,
}

impl FsContextInode {
    pub fn new(flags: FsopenFlags, fs_name: alloc::string::String) -> Arc<Self> {
        let purpose = super::flags::FsContextPurpose::FS_CONTEXT_FOR_MOUNT.bits();
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            flags,
            context: Mutex::new(FsContext::new(fs_name, purpose)),
            wakers: Mutex::new(Vec::new()),
        })
    }

    /// Execute a configuration command
    pub fn execute_command(&self, cmd: FsConfigCommand) -> SysResult<()> {
        let mut context = self.context.lock();

        // log::error!("execute c: {}", cmd.cmd);
        match cmd.cmd {
            c if c == super::flags::FsConfigCmd::FSCONFIG_SET_STRING.bits() => {
                if let (Some(key), Some(value)) = (cmd.key, cmd.value) {
                    let param = match value {
                        super::event::FsParameterValue::String(s) => {
                            super::event::FsParameter::new_string(key, s)
                        }
                        _ => return Err(SysError::EINVAL),
                    };
                    context.add_parameter(param).map_err(|_| SysError::EINVAL)?;
                } else {
                    return Err(SysError::EINVAL);
                }
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_SET_BINARY.bits() => {
                if let (Some(key), Some(value)) = (cmd.key, cmd.value) {
                    let param = match value {
                        super::event::FsParameterValue::Blob(data) => {
                            super::event::FsParameter::new_blob(key, data)
                        }
                        _ => return Err(SysError::EINVAL),
                    };
                    context.add_parameter(param).map_err(|_| SysError::EINVAL)?;
                } else {
                    return Err(SysError::EINVAL);
                }
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_SET_FLAG.bits() => {
                if let Some(key) = cmd.key {
                    let param = super::event::FsParameter::new_flag(key);
                    context.add_parameter(param).map_err(|_| SysError::EINVAL)?;
                } else {
                    return Err(SysError::EINVAL);
                }
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_SET_PATH.bits() => {
                if let (Some(key), Some(value)) = (cmd.key, cmd.value) {
                    let param = match value {
                        super::event::FsParameterValue::Path(path) => {
                            super::event::FsParameter::new_path(key, path)
                        }
                        _ => return Err(SysError::EINVAL),
                    };
                    context.add_parameter(param).map_err(|_| SysError::EINVAL)?;
                } else {
                    return Err(SysError::EINVAL);
                }
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_SET_FD.bits() => {
                if let Some(key) = cmd.key {
                    let param = super::event::FsParameter::new_fd(key, cmd.aux);
                    context.add_parameter(param).map_err(|_| SysError::EINVAL)?;
                } else {
                    return Err(SysError::EINVAL);
                }
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_CMD_CREATE.bits() => {
                log::error!("FSCONFIG_CMD_CREATE");
                context.create_filesystem().map_err(|_| SysError::EINVAL)?;
                self.wake_all_readers();
            }
            c if c == super::flags::FsConfigCmd::FSCONFIG_CMD_RECONFIGURE.bits() => {
                context
                    .reconfigure_filesystem()
                    .map_err(|_| SysError::EINVAL)?;
                self.wake_all_readers();
            }
            _ => return Err(SysError::EINVAL),
        }

        Ok(())
    }

    /// Wake all waiting readers
    fn wake_all_readers(&self) {
        let mut wakers = self.wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    /// Register a waker for notifications
    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.wakers.lock();
        if !wakers.iter().any(|w| w.will_wake(&waker)) {
            wakers.push(waker);
        }
    }

    /// Read error log from the context
    pub fn read_error_log(&self, buf: &mut [u8]) -> SysResult<usize> {
        let context = self.context.lock();
        let log = context.get_error_log();

        if log.is_empty() {
            if self.flags.contains(FsopenFlags::FSOPEN_CLOEXEC) {
                return Err(SysError::ENODATA);
            } else {
                return Ok(0);
            }
        }

        let log_bytes = log.as_bytes();
        let copy_len = core::cmp::min(buf.len(), log_bytes.len());
        buf[..copy_len].copy_from_slice(&log_bytes[..copy_len]);

        Ok(copy_len)
    }

    /// Get the current filesystem context (for fsmount)
    pub fn get_context(&self) -> FsContext {
        self.context.lock().clone()
    }

    /// Get flags
    pub fn get_flags(&self) -> FsopenFlags {
        self.flags
    }

    /// Check if filesystem is created and ready for mounting
    pub fn is_ready_for_mount(&self) -> bool {
        let context = self.context.lock();
        context.is_created()
            && context.phase == super::flags::FsContextPhase::FS_CONTEXT_AWAITING_MOUNT.bits()
    }

    /// Get filesystem type name
    pub fn get_fs_type(&self) -> alloc::string::String {
        self.context.lock().filesystem_type().to_string()
    }
}

impl Inode for FsContextInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}
