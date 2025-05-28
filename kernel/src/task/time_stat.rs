use core::time::Duration;

use arch::time::get_time_duration;
use config::time::TIME_SLICE_DUATION;
use time::TMS;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskState {
    KernelMode,
    UserMode,
}

/// `TaskTimeStat` records time data when task runs.
/// When a task is created, it records its create time.
/// When a task switch between user space and kernel space,
/// this struct can record time in each space.
///
/// Also, this struct can record time of a user task running in
/// cpu. If a task runs for a long time, the kernel can make
/// the task give up the cpu relying on this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskTimeStat {
    state: TaskState,
    task_start: Duration,

    utime: Duration,
    stime: Duration,

    cutime: Duration,
    cstime: Duration,

    last_transition: Duration,
    schedule_start_time: Duration,
}

impl Drop for TaskTimeStat {
    fn drop(&mut self) {
        let total = get_time_duration() - self.task_start;
        log::info!("This Task run for {:?}", total);
    }
}

impl TaskTimeStat {
    pub fn new() -> Self {
        let start = get_time_duration();
        Self {
            state: TaskState::KernelMode,
            task_start: start,
            utime: Duration::ZERO,
            stime: Duration::ZERO,
            cutime: Duration::ZERO,
            cstime: Duration::ZERO,
            last_transition: start,
            schedule_start_time: start,
        }
    }

    /// when `update_time()` is called, it will update recorded time
    /// in struct member with distinguishing User/Kernel Mode automatically.  
    fn update_time(&mut self) {
        let now = get_time_duration();
        let elapsed = now - self.last_transition;
        match self.state {
            TaskState::UserMode => self.utime += elapsed,
            TaskState::KernelMode => self.stime += elapsed,
        }
        self.last_transition = now;
    }

    /// switch `TaskTimeStat` to UserMode and update time.
    pub fn switch_to_user(&mut self) {
        self.update_time();
        self.state = TaskState::UserMode;
    }

    /// switch `TaskTimeStat` to KernelMode and update time.
    pub fn switch_to_kernel(&mut self) {
        self.update_time();
        self.state = TaskState::KernelMode;
    }

    /// When a task is scheduled again, it will call this function
    /// to record start time.
    pub fn record_switch_in(&mut self) {
        self.last_transition = get_time_duration();
        self.schedule_start_time = get_time_duration();
    }

    /// `user_and_system_time` can get user space running time and
    /// kernel space running time after updating time.
    pub fn user_and_system_time(&self) -> (Duration, Duration) {
        let mut final_utime = self.utime;
        let mut final_stime = self.stime;

        // Add time since last transition
        let now = get_time_duration();
        let elapsed = now - self.last_transition;
        match self.state {
            TaskState::UserMode => final_utime += elapsed,
            TaskState::KernelMode => final_stime += elapsed,
        }

        (final_utime, final_stime)
    }

    pub fn update_child_time(&mut self, (utime, stime): (Duration, Duration)) {
        self.cutime += utime;
        self.cstime += stime;
    }

    pub fn child_user_system_time(&self) -> (Duration, Duration) {
        (self.cutime, self.cstime)
    }

    /// `user_time` can get user space running time from
    /// `user_and_system_time`
    #[inline]
    pub fn user_time(&self) -> Duration {
        self.user_and_system_time().0
    }

    /// `sys_time` can get kernel space running time from
    /// `user_and_system_time`
    #[inline]
    pub fn kernel_time(&self) -> Duration {
        self.user_and_system_time().1
    }

    /// `cpu_time` is the sum of user and kernel space time.
    #[inline]
    pub fn cpu_time(&self) -> Duration {
        let (utime, stime) = self.user_and_system_time();
        utime + stime
    }

    /// `schedule_time_out` checks whether a task runs for a long time.
    /// The criterion for timeout is [`TIME_SLICE_DUATION`]. When current
    /// time is longer than last schedule time + TIME_SLICE_DUATION, this function
    /// will return true and notify kernel to schedule another task.
    pub fn schedule_time_out(&self) -> bool {
        get_time_duration() - self.schedule_start_time >= TIME_SLICE_DUATION
    }
}

impl Default for TaskTimeStat {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&TaskTimeStat> for TMS {
    fn from(tts: &TaskTimeStat) -> Self {
        let (utime, stime) = tts.user_and_system_time();
        let (cutime, cstime) = tts.child_user_system_time();
        Self {
            tms_utime: utime.as_micros() as usize,
            tms_stime: stime.as_micros() as usize,
            tms_cutime: cutime.as_micros() as usize,
            tms_cstime: cstime.as_micros() as usize,
        }
    }
}
