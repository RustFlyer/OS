use alloc::{boxed::Box, vec::Vec};
use async_trait::async_trait;
use config::{
    inode::InodeMode,
    vfs::{OpenFlags, PollEvents},
};
use core::{
    sync::atomic::{AtomicU64, Ordering},
    task::Waker,
};
use mutex::SpinNoIrqLock;
use osfuture::{suspend_now, take_waker};
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
    inode::Inode,
    sys_root_dentry,
};

use crate::simple::{anon::AnonDentry, inode::SimpleInode};

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct EventFdFlags: u32  {
        const SEMAPHORE  = 0x1;
        const NONBLOCK  = 0x800;
        const CLOEXEC  = 0x80000;
    }
}

pub struct EventFdFile {
    pub(crate) meta: FileMeta,
    value: AtomicU64,
    flags: u32,
    read_waiters: SpinNoIrqLock<Vec<Waker>>,
    write_waiters: SpinNoIrqLock<Vec<Waker>>,
}

impl EventFdFile {
    pub fn new(initval: u64, flags: u32) -> Self {
        let dentry = AnonDentry::new("eventfd");
        let inode = SimpleInode::new(sys_root_dentry().superblock().unwrap());
        inode.set_mode(InodeMode::CHAR);
        dentry.set_inode(inode);

        let f = Self {
            meta: FileMeta::new(dentry),
            value: AtomicU64::new(initval),
            flags: flags,
            read_waiters: SpinNoIrqLock::new(Vec::new()),
            write_waiters: SpinNoIrqLock::new(Vec::new()),
        };
        f.set_flags(OpenFlags::O_RDWR);

        f
    }
}

impl EventFdFile {
    fn wake_all_readers(&self) {
        let mut waiters = self.read_waiters.lock();
        for waker in waiters.drain(..) {
            waker.wake();
        }
    }

    fn wake_all_writers(&self) {
        let mut waiters = self.write_waiters.lock();
        for waker in waiters.drain(..) {
            waker.wake();
        }
    }
}

#[async_trait]
impl File for EventFdFile {
    fn meta(&self) -> &vfs::file::FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        loop {
            let val = self.value.load(Ordering::Acquire);
            if val == 0 {
                if self.flags & 0x800 != 0 {
                    // EFD_NONBLOCK
                    return Err(SysError::EAGAIN);
                }
                let waker = take_waker().await;
                self.read_waiters.lock().push(waker);
                suspend_now().await;
                continue;
                // return Err(SysError::EAGAIN);
            }

            let to_read = if self.flags & 0x1 != 0 { 1 } else { val }; // EFD_SEMAPHORE
            if self
                .value
                .compare_exchange(val, val - to_read, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                buf[..8].copy_from_slice(&to_read.to_ne_bytes());
                self.wake_all_writers();
                return Ok(8);
            }
        }
    }

    async fn base_write(&self, buf: &[u8], _pos: usize) -> SysResult<usize> {
        let add = u64::from_ne_bytes(buf[..8].try_into().unwrap());
        loop {
            let val = self.value.load(Ordering::Acquire);
            if val > u64::MAX - add {
                if self.flags & 0x800 != 0 {
                    return Err(SysError::EAGAIN);
                }
                let waker = take_waker().await;
                self.write_waiters.lock().push(waker);
                suspend_now().await;
                continue;
                // return Err(SysError::EAGAIN);
            }

            if self
                .value
                .compare_exchange(val, val + add, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.wake_all_readers();
                return Ok(8);
            }
        }
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        let val = self.value.load(Ordering::Acquire);
        if events.contains(PollEvents::IN) && val > 0 {
            res |= PollEvents::IN;
        }
        if events.contains(PollEvents::OUT) && val < u64::MAX {
            res |= PollEvents::OUT;
        }
        res
    }
}
