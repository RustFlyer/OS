use alloc::sync::Arc;

use async_trait::async_trait;

use systype::error::SyscallResult;
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::{RtcTime, ioctl::RtcIoctlCmd};

pub struct RtcFile {
    meta: FileMeta,
}

impl RtcFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

#[async_trait]
impl File for RtcFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        let cmd = (cmd as u32) as u64;
        let Some(cmd) = RtcIoctlCmd::from_repr(cmd as u64) else {
            log::error!("[TtyFile::ioctl] cmd {cmd:#x} not included");
            unimplemented!()
        };
        match cmd {
            RtcIoctlCmd::RTC_RD_TIME => unsafe {
                *(arg as *mut RtcTime) = RtcTime::default();
            },
            _ => {
                log::error!("not implement rtc ioctl {:?}", cmd);
            }
        }

        Ok(0)
    }
}
