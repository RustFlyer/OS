use core::{
    pin::Pin,
    task::{Context, Poll},
};

use alloc::sync::Arc;
use config::vfs::PollEvents;
use vfs::{file::FileMeta, inode::Inode};

use crate::simple::dentry::SimpleDentry;

use super::inode::PipeInode;

pub struct PipeReadFile {
    pub(crate) meta: FileMeta,
}

impl PipeReadFile {
    pub fn new(inode: Arc<PipeInode>) -> Arc<Self> {
        let dentry = SimpleDentry::new("r", Some(inode), None);
        let meta = FileMeta::new(dentry);
        Arc::new(Self { meta })
    }
}

impl Drop for PipeReadFile {
    fn drop(&mut self) {
        let pipe = self
            .meta
            .dentry
            .inode()
            .unwrap()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        log::info!(
            "[PipeReadFile::drop] pipe ino {} read end is closed",
            pipe.get_meta().ino
        );
        let mut inner = pipe.inner.lock();
        inner.is_read_closed = true;
        while let Some(waker) = inner.write_waker.pop_front() {
            waker.wake();
        }
    }
}

pub(crate) struct PipeReadPollFuture {
    events: PollEvents,
    pipe: Arc<PipeInode>,
}

impl PipeReadPollFuture {
    pub fn new(pipe: Arc<PipeInode>, events: PollEvents) -> Self {
        Self { pipe, events }
    }
}

impl Future for PipeReadPollFuture {
    type Output = PollEvents;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.pipe.inner.lock();
        let mut res = PollEvents::empty();
        if self.events.contains(PollEvents::IN) && !inner.ring_buffer.is_empty() {
            res |= PollEvents::IN;
            Poll::Ready(res)
        } else {
            if inner.is_write_closed {
                res |= PollEvents::HUP;
                return Poll::Ready(res);
            }
            inner.read_waker.push_back(cx.waker().clone());
            Poll::Pending
        }
    }
}
