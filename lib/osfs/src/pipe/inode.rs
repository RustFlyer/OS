use core::task::Waker;

use alloc::{collections::vec_deque::VecDeque, sync::Arc};
use config::{
    inode::{InodeMode, InodeType},
    mm::PAGE_SIZE,
};
use mutex::SpinNoIrqLock;
use systype::error::SysResult;
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::ringbuffer::RingBuffer;
pub const PIPE_BUF_LEN: usize = 16 * PAGE_SIZE;
pub struct PipeInode {
    meta: InodeMeta,
    pub(crate) inner: SpinNoIrqLock<PipeInodeInner>,
}

pub struct PipeInodeInner {
    pub(crate) is_write_closed: bool,
    pub(crate) is_read_closed: bool,
    pub(crate) ring_buffer: RingBuffer,
    // WARN: `Waker` may not wake the task exactly, it may be abandoned.
    // Rust only guarentees that waker will wake the task from the last poll where the waker is
    // passed in.
    // FIXME: `sys_ppoll` and `sys_pselect6` may return because of other wake ups
    // while the waker registered here is not removed.
    pub(crate) read_waker: VecDeque<Waker>,
    pub(crate) write_waker: VecDeque<Waker>,
}

impl PipeInode {
    pub fn new(len: usize) -> Arc<Self> {
        let meta = InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap());
        let inner = SpinNoIrqLock::new(PipeInodeInner {
            is_write_closed: false,
            is_read_closed: false,
            ring_buffer: RingBuffer::new(len),
            read_waker: VecDeque::new(),
            write_waker: VecDeque::new(),
        });
        let inode = Arc::new(Self { meta, inner });

        inode.set_inotype(InodeType::from(InodeMode::FIFO));
        inode.set_size(PIPE_BUF_LEN);
        inode
    }
}

impl Inode for PipeInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: inner.size as u64,
            st_blksize: 0,
            __pad2: 0,
            st_blocks: 0 as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
