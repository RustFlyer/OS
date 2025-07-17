use core::{task::Waker, time::Duration};

use crate::simple::{anon::AnonDentry, inode::SimpleInode};

use super::flag::TimerFdFlags;
use alloc::boxed::Box;
use async_trait::async_trait;
use config::{inode::InodeMode, vfs::OpenFlags};
use mutex::SpinNoIrqLock;
use osfuture::take_waker;
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
    inode::Inode,
    sys_root_dentry,
};

// todo!: it should count in timer.
pub struct TimerFdFile {
    pub(crate) meta: FileMeta,
    pub clockid: i32,
    pub flags: TimerFdFlags,
    inner: SpinNoIrqLock<TimerState>,
    waker: Waker,
}

#[derive(Default)]
struct TimerState {
    it_value: Option<Duration>,    // ddl
    it_interval: Option<Duration>, // rep
    expired_count: u64,            // unread count
    armed: bool,
}

impl TimerFdFile {
    pub async fn new(clockid: i32, flags: TimerFdFlags) -> Self {
        let dentry = AnonDentry::new("timerfd");
        let inode = SimpleInode::new(sys_root_dentry().superblock().unwrap());
        inode.set_mode(InodeMode::CHAR);
        dentry.set_inode(inode);

        let f = Self {
            meta: FileMeta::new(dentry),
            clockid,
            flags,
            inner: SpinNoIrqLock::new(TimerState::default()),
            waker: take_waker().await,
        };
        f.set_flags(OpenFlags::O_RDWR);

        f
    }
}

#[async_trait]
impl File for TimerFdFile {
    fn meta(&self) -> &vfs::file::FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        let mut inner = self.inner.lock();
        if inner.expired_count == 0 {
            if self.flags.contains(TimerFdFlags::NONBLOCK) {
                return Err(SysError::EAGAIN);
            } else {
                todo!()
                // block to wait until expired_count>0
            }
        }
        let count = inner.expired_count;
        inner.expired_count = 0;
        let bytes = count.to_ne_bytes();
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        Ok(n)
    }
}
