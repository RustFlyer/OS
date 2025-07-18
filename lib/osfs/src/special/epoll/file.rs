use core::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::simple::dentry::SimpleDentry;

use super::event::{EpollEvent, EpollInner};
use alloc::vec::Vec;
use config::vfs::{EpollEvents, PollEvents};
use mutex::SpinNoIrqLock;
use vfs::file::{File, FileMeta};

pub struct EpollFile {
    pub(crate) meta: FileMeta,
    pub inner: SpinNoIrqLock<EpollInner>,
}

impl EpollFile {
    pub fn new() -> Self {
        let dentry = SimpleDentry::new("epoll", None, None);
        Self {
            meta: FileMeta::new(dentry),
            inner: SpinNoIrqLock::new(EpollInner::new()),
        }
    }
}

impl File for EpollFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }
}

pub struct EpollFuture {
    pub inner: EpollInner,
    pub maxevents: usize,
}

impl EpollFuture {
    pub fn new(inner: EpollInner, maxevents: usize) -> Self {
        Self { inner, maxevents }
    }
}

/// When epoll attends to poll, it will call future.await to poll files.
impl Future for EpollFuture {
    type Output = (EpollInner, Vec<EpollEvent>);

    /// Return vec of futures that are ready. Return `Poll::Pending` if
    /// no futures are ready.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        // 1. poll files
        let inner = &this.inner;
        let mut ret_vec = Vec::with_capacity(inner.entries.len());
        for entry in inner.entries.iter() {
            let eevent = entry.event;
            let events = eevent.events;
            let file = entry.file.clone();

            let result =
                unsafe { Pin::new_unchecked(&mut file.poll(PollEvents::from(events))).poll(cx) };

            // 2. check results of file poll.
            match result {
                Poll::Pending => unreachable!(),
                Poll::Ready(result) => {
                    let mut ee = entry.event;
                    ee.events = EpollEvents::from(result);
                    if !result.is_empty() {
                        ret_vec.push(ee)
                    }
                }
            }
        }

        // 3. If anyone(or multi) is finished, return the result. Otherwise, return pending.
        if ret_vec.len() > 0 {
            if ret_vec.len() > this.maxevents {
                ret_vec.truncate(this.maxevents);
            }
            Poll::Ready((inner.clone(), ret_vec))
        } else {
            log::debug!("[EpollFuture] waiting..");
            Poll::Pending
        }
    }
}
