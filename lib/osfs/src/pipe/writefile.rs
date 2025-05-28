use alloc::boxed::Box;

use async_trait::async_trait;
use config::vfs::PollEvents;
use osfuture::take_waker;
use systype::error::{SysError, SysResult};
use vfs::{
    file::{File, FileMeta},
    inode::Inode,
};

use super::write::PipeWriteFile;
use crate::pipe::{inode::PipeInode, write::PipeWritePollFuture};

#[async_trait]
impl File for PipeWriteFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        log::info!(
            "[PipeWriteFile::base_write_at] read pipe ino {}",
            pipe.get_meta().ino
        );
        let revents = PipeWritePollFuture::new(pipe.clone(), PollEvents::OUT).await;
        if revents.contains(PollEvents::ERR) {
            return Err(SysError::EPIPE);
        }
        assert!(revents.contains(PollEvents::OUT));
        let mut inner = pipe.inner.lock();
        let len = inner.ring_buffer.write(buf);
        if let Some(waker) = inner.read_waker.pop_front() {
            waker.wake();
        }
        // log::trace!("[Pipe::write] already write buf {buf:?} with data len {len:?}");
        return Ok(len);
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let waker = take_waker().await;
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        let mut inner = pipe.inner.lock();
        let mut res = PollEvents::empty();
        if inner.is_read_closed {
            res |= PollEvents::ERR;
        }
        if events.contains(PollEvents::OUT) && !inner.ring_buffer.is_full() {
            res |= PollEvents::OUT;
        } else {
            inner.write_waker.push_back(waker);
        }
        res
    }
}
