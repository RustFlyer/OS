use crate::pipe::{inode::PipeInode, read::PipeReadPollFuture};
use alloc::boxed::Box;
use async_trait::async_trait;
use config::vfs::PollEvents;
use osfuture::take_waker;
use systype::SysResult;
use vfs::file::{File, FileMeta};

use super::read::PipeReadFile;

#[async_trait]
impl File for PipeReadFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        // log::info!(
        //     "[PipeReadFile::base_read_at] read pipe ino {}",
        //     pipe.get_meta().ino
        // );
        let events = PollEvents::IN;
        let revents = PipeReadPollFuture::new(pipe.clone(), events).await;
        if revents.contains(PollEvents::HUP) {
            return Ok(0);
        }
        assert!(revents.contains(PollEvents::IN));
        let mut inner = pipe.inner.lock();

        let len = inner.ring_buffer.read(buf);
        if let Some(waker) = inner.write_waker.pop_front() {
            waker.wake();
        }
        return Ok(len);
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        let waker = take_waker().await;
        let mut inner = pipe.inner.lock();
        let mut res = PollEvents::empty();
        if inner.is_write_closed {
            res |= PollEvents::HUP;
        }
        if events.contains(PollEvents::IN) && !inner.ring_buffer.is_empty() {
            res |= PollEvents::IN;
        } else {
            inner.read_waker.push_back(waker);
        }
        res
    }
}
