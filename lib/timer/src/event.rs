use core::{fmt::Debug, task::Waker};

use alloc::sync::Arc;

use crate::TimerState;

pub trait IEvent: Send + Sync + Debug {
    fn callback(self: Arc<Self>) -> TimerState;
}

#[derive(Debug)]
pub struct WakerEvent {
    pub(crate) waker: Waker,
}

impl IEvent for WakerEvent {
    fn callback(self: Arc<Self>) -> TimerState {
        self.waker.wake_by_ref();
        TimerState::Cancelled
    }
}
