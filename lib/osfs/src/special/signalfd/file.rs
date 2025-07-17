use alloc::boxed::Box;
use async_trait::async_trait;
use config::vfs::PollEvents;
use core::sync::atomic::{AtomicU64, Ordering};
use systype::error::SysResult;
use vfs::file::{File, FileMeta};

use crate::simple::anon::AnonDentry;

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct SignalFdFileFlags: u32  {
        const SEMAPHORE  = 0x1;
        const NONBLOCK  = 0x800;
        const CLOEXEC  = 0x80000;
    }
}

pub struct SignalFdFile {
    pub(crate) meta: FileMeta,
    value: AtomicU64,
}

impl SignalFdFile {
    pub fn new(initval: u64, flags: u32) -> Self {
        let dentry = AnonDentry::new("signalfd");
        Self {
            meta: FileMeta::new(dentry),
            value: AtomicU64::new(initval),
        }
    }
}

#[async_trait]
impl File for SignalFdFile {
    fn meta(&self) -> &vfs::file::FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        Ok(0)
    }

    async fn base_write(&self, buf: &[u8], _pos: usize) -> SysResult<usize> {
        Ok(0)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        events
    }
}
