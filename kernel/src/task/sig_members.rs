use core::fmt::{Debug, write};

use alloc::collections::vec_deque::VecDeque;
use bitflags::bitflags;

use crate::task::{TaskState, signal::sig_info::*};

use super::Task;

impl Task {
    ///BODGE: regardless of threads in a group for now.
    pub fn receive_siginfo(&self, si: SigInfo) {
        self.recv(si)
    }

    fn recv(&self, si: SigInfo) {
        log::info!(
            "[Task::recv] tid {} recv {si:?} {:?}",
            self.tid(),
            self.sig_handlers_mut().lock().get(si.sig)
        );

        let manager = self.sig_manager_mut();
        manager.add(si);
        if manager.should_wake.contain_signal(si.sig) && self.is_in_state(TaskState::Interruptable)
        {
            log::info!("[Task::recv] tid {} has been woken", self.tid());
            self.wake();
        } else {
            log::info!(
                "[Task::recv] tid {} hasn't been woken, should_wake {:?}, state {:?}",
                self.tid(),
                manager.should_wake,
                self.get_state()
            );
        }
    }
}

pub struct SigManager {
    /// 接收到的所有信号
    pub queue: VecDeque<SigInfo>,
    /// 比特位的内容代表是否收到信号，主要用来防止queue收到重复信号
    pub bitmap: SigSet,
    /// 如果在receive_siginfo的时候收到的信号位于should_wake信号集合中，
    /// 且task的wake存在，那么唤醒task
    pub should_wake: SigSet,
}

impl Debug for SigManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "sigmanager: {}", self.bitmap.bits())
    }
}

impl SigManager {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            bitmap: SigSet::empty(),
            should_wake: SigSet::empty(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn add(&mut self, si: SigInfo) {
        if !self.bitmap.contain_signal(si.sig) {
            self.bitmap.add_signal(si.sig);
            self.queue.push_back(si);
        }
    }

    /// Dequeue a signal and return the SigInfo to the caller
    pub fn dequeue_signal(&mut self, mask: &SigSet) -> Option<SigInfo> {
        let mut x = self.bitmap & (!*mask);
        let mut sig = Sig::from_i32(0);
        if !x.is_empty() {
            if !(x & SigSet::SYNCHRONOUS_MASK).is_empty() {
                x &= SigSet::SYNCHRONOUS_MASK;
            }
            sig = Sig::from_i32((x.bits().trailing_zeros() + 1) as _);
        }
        if sig.raw() == 0 {
            return None;
        }
        for i in 0..self.queue.len() {
            if self.queue[i].sig == sig {
                self.bitmap.remove_signal(sig);
                return self.queue.remove(i);
            }
        }
        log::error!("[dequeue_signal] I suppose it won't go here");
        return None;
    }

    /// Dequeue a sepcific signal in `expect` even if it is blocked and return
    /// the SigInfo to the caller
    pub fn dequeue_expect(&mut self, expect: SigSet) -> Option<SigInfo> {
        let x = self.bitmap & expect;
        if x.is_empty() {
            return None;
        }
        for i in 0..self.queue.len() {
            let sig = self.queue[i].sig;
            if x.contain_signal(sig) {
                self.bitmap.remove_signal(sig);
                return self.queue.remove(i);
            }
        }
        log::error!("[dequeue_expect] I suppose it won't go here");
        None
    }

    pub fn get_expect(&mut self, expect: SigSet) -> Option<SigInfo> {
        let x = self.bitmap & expect;
        if x.is_empty() {
            return None;
        }
        for i in 0..self.queue.len() {
            let si = self.queue[i];
            if x.contain_signal(si.sig) {
                return Some(si);
            }
        }
        log::error!("[get_expect] I suppose it won't go here");
        None
    }

    pub fn has_expect_signals(&self, expect: SigSet) -> bool {
        !(expect & self.bitmap).is_empty()
    }
}

pub struct SigHandlers {
    /// 注意信号编号与数组索引有1个offset，因此在Sig中有个index()函数负责-1
    actions: [Action; NSIG],
    /// 一个位掩码，如果为1表示该信号是用户定义的，如果为0表示默认。
    /// (实际上可以由actions间接得出来，这里只是存了一个快速路径)
    bitmap: SigSet,
}

impl Debug for SigHandlers {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.bitmap.bits())
    }
}

impl SigHandlers {
    pub fn new() -> Self {
        Self {
            actions: core::array::from_fn(|signo| Action::new((signo + 1).into())),
            bitmap: SigSet::empty(),
        }
    }

    pub fn get(&self, sig: Sig) -> Action {
        debug_assert!(sig.is_valid());
        self.actions[sig.index()]
    }

    /// update actions and bitmap in actions in sig_handlers
    pub fn update(&mut self, sig: Sig, new: Action) {
        debug_assert!(!sig.is_kill_or_stop());
        self.actions[sig.index()] = new;
        match new.atype {
            ActionType::User { .. } | ActionType::Kill => self.bitmap.add_signal(sig),
            _ => self.bitmap.remove_signal(sig),
        }
    }

    /// it is used in execve because it changed the memory
    pub fn reset_user_defined(&mut self) {
        for n in 0..NSIG {
            match self.actions[n].atype {
                ActionType::User { .. } => {
                    self.actions[n].atype = ActionType::default(Sig::from_i32((n + 1) as _));
                }
                _ => {}
            }
        }
        self.bitmap = SigSet::empty();
    }

    pub fn bitmap(&self) -> SigSet {
        self.bitmap
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ActionType {
    Ignore,
    Kill,
    Stop,
    Cont,
    User { entry: usize },
}

impl ActionType {
    pub fn default(sig: Sig) -> Self {
        match sig {
            Sig::SIGCHLD | Sig::SIGURG | Sig::SIGWINCH => ActionType::Ignore,
            Sig::SIGSTOP | Sig::SIGTSTP | Sig::SIGTTIN | Sig::SIGTTOU => ActionType::Stop,
            Sig::SIGCONT => ActionType::Cont,
            _ => ActionType::Kill,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Action {
    pub atype: ActionType,
    // 一个位掩码，每个比特位对应于系统中的一个信号。它用于在处理程序例程执行期间阻塞其他信号。
    // 在例程结束后，内核会重置其值，回复到信号处理之前的原值
    pub flags: SigActionFlag,
    pub mask: SigSet,
}

bitflags! {
    #[derive(Default, Copy, Clone, Debug)]
    pub struct SigActionFlag : usize {
        const SA_NOCLDSTOP = 1;
        const SA_NOCLDWAIT = 2;
        const SA_SIGINFO = 4;
        const SA_ONSTACK = 0x08000000;
        const SA_RESTART = 0x10000000;
        const SA_NODEFER = 0x40000000;
        const SA_RESETHAND = 0x80000000;
        const SA_RESTORER = 0x04000000;
    }
}

impl Action {
    pub fn new(sig: Sig) -> Self {
        let atype = ActionType::default(sig);
        Self {
            atype,
            flags: Default::default(),
            mask: SigSet::empty(),
        }
    }
}

/// 信号栈是为信号处理程序执行提供的专用栈空间.它通常包含以下内容:
/// 1.信号上下文：这是信号处理程序运行时的上下文信息，包括所有寄存器的值、
/// 程序计数器（PC）、栈指针等。它使得信号处理程序可以访问到被中断的程序的状态，
/// 并且在处理完信号后能够恢复这个状态，继续执行原程序。
/// 2.信号信息（siginfo_t）：这个结构提供了关于信号的具体信息，如信号的来源、
/// 产生信号的原因等。 3.调用栈帧：如果信号处理程序调用了其他函数，
/// 那么这些函数的栈帧也会被压入信号栈。每个栈帧通常包含了函数参数、
/// 局部变量以及返回地址。 4.信号处理程序的返回地址：当信号处理程序完成执行后，
/// 系统需要知道从哪里返回继续执行，因此信号栈上会保存一个返回地址。
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SignalStack {
    /// Base address of stack
    pub ss_sp: usize,
    /// Flags
    pub ss_flags: i32,
    /// Number of bytes in stack
    pub ss_size: usize,
}

impl Default for SignalStack {
    fn default() -> Self {
        SignalStack {
            ss_sp: 0usize.into(),
            ss_flags: 0,
            ss_size: 0,
        }
    }
}

impl SignalStack {
    pub fn get_stack_top(&self) -> usize {
        self.ss_sp + self.ss_size
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SigContext {
    pub flags: usize,
    /// 当前上下文返回时将恢复执行的下一个上下文的指针
    pub link: usize,
    // 当前上下文使用的栈信息,包含栈的基址、大小等信息
    pub stack: SignalStack,
    // 当前上下文活跃时被阻塞的信号集
    pub mask: SigSet,
    // don't know why, struct need to be exact the same with musl libc
    pub sig: [usize; 16],
    // common register
    pub user_reg: [usize; 32],
    //
    pub fpstate: [usize; 66],
}
