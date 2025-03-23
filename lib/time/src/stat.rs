use arch::riscv64::time::get_time_duration;
use config::time::TIME_SLICE_DUATION;
use core::time::Duration;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TaskState {
    KernelMode,
    UserMode,
}

/// 任务计时器结构体
///
/// 表示一个任务的计时器，包含任务状态、任务开始时间、用户时间、系统时间、子用户时间、子系统时间和最后一次状态切换时间
#[allow(unused)]
#[derive(Debug, Clone)]
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

    fn update_time(&mut self) {
        let now = get_time_duration();
        let elapsed = now - self.last_transition;
        match self.state {
            TaskState::UserMode => self.utime += elapsed,
            TaskState::KernelMode => self.stime += elapsed,
        }
        self.last_transition = now;
    }

    pub fn switch_to_user(&mut self) {
        self.update_time();
        self.state = TaskState::UserMode;
    }

    pub fn switch_to_kernel(&mut self) {
        self.update_time();
        self.state = TaskState::KernelMode;
    }

    pub fn record_switch_in(&mut self) {
        self.last_transition = get_time_duration();
        self.schedule_start_time = get_time_duration();
    }

    pub fn record_switch_out(&mut self) {
        self.update_time();
    }

    pub fn record_trap(&mut self) {
        self.switch_to_kernel();
    }

    pub fn record_trap_return(&mut self) {
        self.switch_to_user();
    }

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

    pub fn child_user_system_time(&self) -> (Duration, Duration) {
        (self.cutime, self.cstime)
    }

    #[inline]
    pub fn user_time(&self) -> Duration {
        self.user_and_system_time().0
    }

    #[inline]
    pub fn sys_time(&self) -> Duration {
        self.user_and_system_time().1
    }

    #[inline]
    pub fn cpu_time(&self) -> Duration {
        let (utime, stime) = self.user_and_system_time();
        utime + stime
    }

    pub fn update_child_time(&mut self, (utime, stime): (Duration, Duration)) {
        self.cutime += utime;
        self.cstime += stime;
    }

    pub fn schedule_time_out(&self) -> bool {
        get_time_duration() - self.schedule_start_time >= TIME_SLICE_DUATION
    }
}
