use core::task::Waker;

use alloc::{boxed::Box, collections::vec_deque::VecDeque};
use async_trait::async_trait;
use config::{inode::InodeMode, vfs::OpenFlags};
use mutex::SpinNoIrqLock;
use osfuture::{suspend_now, take_waker};
use signal::{SigInfo, SigSet};
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
    inode::Inode,
    sys_root_dentry,
};

use crate::{
    pselect::has_expected_signal,
    simple::{anon::AnonDentry, inode::SimpleInode},
};

use super::{flag::SignalFdFlags, info::SignalfdSiginfo};

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
    pub mask: SpinNoIrqLock<SigSet>,         // watched sig set
    queue: SpinNoIrqLock<VecDeque<SigInfo>>, // coming signals
    flags: SignalFdFlags,
    waker: Waker,
}

impl SignalFdFile {
    pub async fn new(mask: SigSet, flags: SignalFdFlags) -> Self {
        let dentry = AnonDentry::new("signalfd");
        let inode = SimpleInode::new(sys_root_dentry().superblock().unwrap());
        inode.set_mode(InodeMode::CHAR);
        dentry.set_inode(inode);

        let f = Self {
            meta: FileMeta::new(dentry),
            mask: SpinNoIrqLock::new(mask),
            queue: SpinNoIrqLock::new(VecDeque::new()),
            flags,
            waker: take_waker().await,
        };

        f.set_flags(OpenFlags::O_RDWR);
        f
    }

    pub fn notify_signal(&self, siginfo: SigInfo) {
        let mut queue = self.queue.lock();
        queue.push_back(siginfo);

        if !self.flags.contains(SignalFdFlags::NONBLOCK) {
            self.waker.wake_by_ref();
        }
    }

    pub fn update_mask(&self, mask: SigSet) {
        *self.mask.lock() = mask;
    }
}

#[async_trait]
impl File for SignalFdFile {
    fn meta(&self) -> &vfs::file::FileMeta {
        &self.meta
    }

    /// read signal in queue and push them into buf
    /// when queue is none, fd will suspend(BLOCK) or return AGIAN Err(NONBLOCK).
    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        loop {
            {
                let mut queue = self.queue.lock();
                if let Some(siginfo) = queue.pop_front() {
                    let sfdinfo = SignalfdSiginfo::from(&siginfo);
                    let sfdinfo_bytes = unsafe {
                        core::slice::from_raw_parts(
                            &sfdinfo as *const SignalfdSiginfo as *const u8,
                            core::mem::size_of::<SignalfdSiginfo>(),
                        )
                    };
                    if buf.len() < sfdinfo_bytes.len() {
                        return Err(SysError::EINVAL);
                    }
                    buf[..sfdinfo_bytes.len()].copy_from_slice(sfdinfo_bytes);
                    return Ok(sfdinfo_bytes.len());
                }
            }

            if self.flags.contains(SignalFdFlags::NONBLOCK) {
                return Err(SysError::EAGAIN);
            }

            while !has_expected_signal(*self.mask.lock()) {
                suspend_now().await;
            }
        }
    }
}
