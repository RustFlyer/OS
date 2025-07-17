use core::{
    pin::Pin,
    task::{Context, Poll},
};

use alloc::{sync::Arc, vec::Vec};
use config::vfs::PollEvents;
use crate_interface::call_interface;
use signal::SigSet;
use vfs::file::File;

use crate::fd_table::Fd;

#[crate_interface::def_interface]
pub trait PSFHasSignalIf: Send + Sync {
    fn has_signal() -> bool;
    fn has_expected_signal(sigset: SigSet) -> bool;
}

pub fn _has_signal() -> bool {
    call_interface!(PSFHasSignalIf::has_signal())
}

pub fn has_expected_signal(sigset: SigSet) -> bool {
    call_interface!(PSFHasSignalIf::has_expected_signal(sigset))
}

pub type FilePollRet = (Fd, PollEvents, Arc<dyn File>);

pub struct PSelectFuture {
    pub polls: Vec<FilePollRet>,
}

impl PSelectFuture {
    pub fn new(polls: Vec<FilePollRet>) -> Self {
        Self { polls }
    }
}

impl Future for PSelectFuture {
    type Output = Vec<(Fd, PollEvents)>;

    /// Return vec of futures that are ready. Return `Poll::Pending` if
    /// no futures are ready.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        // 1. poll files
        let mut ret_vec = Vec::with_capacity(this.polls.len());
        for (fd, events, file) in this.polls.iter() {
            let result = unsafe { Pin::new_unchecked(&mut file.poll(*events)).poll(cx) };

            // 2. check results of file poll.
            match result {
                Poll::Pending => unreachable!(),
                Poll::Ready(result) => {
                    if !result.is_empty() {
                        ret_vec.push((*fd, result))
                    }
                }
            }
        }

        // 3. If anyone(or multi) is finished, return the result. Otherwise, return pending.
        if ret_vec.len() > 0 {
            Poll::Ready(ret_vec)
        } else {
            log::debug!("[PSelectFuture] waiting..");
            Poll::Pending
        }
    }
}
