use arch::riscv64::time::get_time_duration;
use core::time::Duration;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TaskState {
    KernelMode,
    UserMode,
}

/// 任务计时器结构体
///
/// 表示一个任务的计时器，包含任务状态、任务开始时间、用户时间、系统时间、子用户时间、子系统时间和最后一次状态切换时间
#[derive(Debug, Clone)]
pub struct TaskTimeStat {
    state: TaskState,
    task_start: Duration,
    utime: Duration,
    stime: Duration,
    cutime: Duration,
    cstime: Duration,
    last_transition: Duration,
}

impl TaskTimeStat {
    /// 创建一个新的任务计时器
    ///
    /// 返回一个新的任务计时器，初始状态为内核模式，开始时间为当前时间
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
        }
    }

    /// 更新任务时间
    ///
    /// 更新任务时间，根据任务状态更新用户时间或系统时间
    fn update_time(&mut self) {
        let now = get_time_duration();
        let elapsed = now - self.last_transition;
        match self.state {
            TaskState::UserMode => self.utime += elapsed,
            TaskState::KernelMode => self.stime += elapsed,
        }
        self.last_transition = now;
    }

    /// 切换到用户模式
    ///
    /// 切换到用户模式，更新任务时间
    pub fn switch_to_user(&mut self) {
        self.update_time();
        self.state = TaskState::UserMode;
    }

    /// 切换到内核模式
    ///
    /// 切换到内核模式，更新任务时间
    pub fn switch_to_kernel(&mut self) {
        self.update_time();
        self.state = TaskState::KernelMode;
    }

    /// 记录状态切换
    ///
    /// 记录状态切换，更新最后一次状态切换时间
    pub fn record_switch_in(&mut self) {
        self.last_transition = get_time_duration();
        self.state = TaskState::KernelMode;
    }

    /// 记录状态切换
    ///
    /// 记录状态切换，更新最后一次状态切换时间
    pub fn record_switch_out(&mut self) {
        self.update_time();
    }

    /// 记录陷阱
    ///
    /// 记录陷阱，切换到内核模式
    pub fn record_trap(&mut self) {
        self.switch_to_kernel();
    }

    /// 记录陷阱返回
    ///
    /// 记录陷阱返回，切换到用户模式
    pub fn record_trap_return(&mut self) {
        self.switch_to_user();
    }

    /// 返回用户时间和系统时间
    ///
    /// 返回用户时间和系统时间
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

    /// 返回子用户时间和系统时间
    ///
    /// 返回子用户时间和系统时间
    pub fn child_user_system_time(&self) -> (Duration, Duration) {
        (self.cutime, self.cstime)
    }

    /// 返回用户时间
    ///
    /// 返回用户时间
    #[inline]
    pub fn user_time(&self) -> Duration {
        self.user_and_system_time().0
    }

    /// 返回系统时间
    ///
    /// 返回系统时间
    #[inline]
    pub fn sys_time(&self) -> Duration {
        self.user_and_system_time().1
    }

    /// 返回CPU时间
    ///
    /// 返回CPU时间
    #[inline]
    pub fn cpu_time(&self) -> Duration {
        let (utime, stime) = self.user_and_system_time();
        utime + stime
    }

    /// 更新子用户和系统时间
    ///
    /// 更新子用户和系统时间
    pub fn update_child_time(&mut self, (utime, stime): (Duration, Duration)) {
        self.cutime += utime;
        self.cstime += stime;
    }
}
