use core::time::Duration;

use alloc::sync::{Arc, Weak};
use bitflags::bitflags;
use timer::{IEvent, TimerState};

use super::{
    Task,
    signal::sig_info::{Sig, SigDetails, SigInfo},
};

#[derive(Default, Debug)]
pub struct RealITimer {
    pub task: Weak<Task>,
    pub id: usize,
}

impl IEvent for RealITimer {
    fn callback(self: Arc<Self>) -> TimerState {
        if let Some(task) = self.task.upgrade() {
            task.with_mut_itimers(|itimers| {
                log::debug!("[RealITimer] IEvent is called");
                let real = &mut itimers[0];

                if real.id != self.id {
                    log::debug!("[RealITimer] IEvent id wrong");
                    return TimerState::Cancelled;
                }

                task.receive_siginfo(SigInfo {
                    sig: Sig::SIGALRM,
                    code: SigInfo::KERNEL,
                    details: SigDetails::None,
                });

                if real.interval == Duration::ZERO {
                    log::debug!("[RealITimer] IEvent is Once");
                    TimerState::Cancelled
                } else {
                    log::debug!("[RealITimer] IEvent wakes next timer");
                    TimerState::Active
                }
            })
        } else {
            TimerState::Cancelled
        }
    }
}

impl Task {
    pub fn get_process_ustime(&self) -> (Duration, Duration) {
        self.with_thread_group(|tg| -> (Duration, Duration) {
            tg.iter()
                .map(|thread| thread.timer_mut().user_and_system_time())
                .reduce(|(acc_utime, acc_stime), (utime, stime)| {
                    (acc_utime + utime, acc_stime + stime)
                })
                .unwrap()
        })
    }

    pub fn get_process_utime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.timer_mut().user_time())
                .reduce(|acc_utime, utime| acc_utime + utime)
                .unwrap()
        })
    }

    pub fn get_process_cputime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.timer_mut().cpu_time())
                .reduce(|acc, cputime| acc + cputime)
                .unwrap()
        })
    }
}
