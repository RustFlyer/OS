use core::{
    pin::Pin,
    task::{Context, Poll},
};

use alloc::sync::Arc;
use config::vfs::PollEvents;
use mutex::SpinNoIrqLock;
use vfs::{file::FileMeta, inode::Inode};

use crate::simple::dentry::SimpleDentry;

use super::inode::PipeInode;

pub struct PipeWriteFile {
    pub(crate) meta: FileMeta,
}

impl PipeWriteFile {
    pub fn new(inode: Arc<dyn Inode>) -> Arc<Self> {
        let dentry = SimpleDentry::new("w", Some(inode), None);
        let meta = FileMeta::new(dentry);
        *meta.flags.lock() = config::vfs::OpenFlags::O_WRONLY;
        Arc::new(Self { meta })
    }
}

// NOTE: `PipeReadFile` is hold by task as `Arc<dyn File>`.
impl Drop for PipeWriteFile {
    fn drop(&mut self) {
        let pipe = self
            .meta
            .dentry
            .inode()
            .unwrap()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        log::info!(
            "[PipeWriteFile::drop] pipe ino {} write end is closed",
            pipe.get_meta().ino
        );
        let mut inner = pipe.inner.lock();
        inner.is_write_closed = true;
        while let Some(waker) = inner.read_waker.pop_front() {
            waker.wake();
        }
    }
}

pub(crate) struct PipeWritePollFuture {
    events: PollEvents,
    pipe: Arc<PipeInode>,
    cnt: SpinNoIrqLock<usize>,
}

impl PipeWritePollFuture {
    pub fn new(pipe: Arc<PipeInode>, events: PollEvents) -> Self {
        Self {
            pipe,
            events,
            cnt: SpinNoIrqLock::new(0),
        }
    }
}

impl Future for PipeWritePollFuture {
    type Output = PollEvents;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        *self.cnt.lock() += 1;
        let mut inner = self.pipe.inner.lock();
        let mut res = PollEvents::empty();
        if inner.is_read_closed {
            res |= PollEvents::ERR;
            return Poll::Ready(res);
        }
        if (self.events.contains(PollEvents::OUT) && !inner.ring_buffer.is_full())
        // || *self.cnt.lock() >= 2
        {
            res |= PollEvents::OUT;
            Poll::Ready(res)
        } else {
            inner.write_waker.push_back(cx.waker().clone());
            log::debug!(
                "[PipeReadPollFuture] buffer is full? {:?} {} or {:?} is not OUT, suspend",
                inner.ring_buffer.is_full(),
                inner.ring_buffer.len(),
                self.events
            );
            Poll::Pending
        }
    }
}
