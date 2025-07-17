#![no_main]
#![no_std]

use core::fmt;

use bitflags::*;

pub const NSIG: usize = 64;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SigInfo {
    pub sig: Sig,
    pub code: i32,
    pub details: SigDetails,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub enum SigDetails {
    None,
    Kill {
        /// sender's pid
        pid: usize,
        /// When calling Linux `pidfd_send_signal` system call, this is the `info`
        /// parameter passed to the signal handler. Otherwise, it is `None`.
        siginfo: Option<LinuxSigInfo>,
    },
    Child {
        /// child's pid
        pid: usize,
    },
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct LinuxSigInfo {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    pub si_trapno: i32,
    pub si_pid: i32,
    pub si_uid: u32,
    pub si_status: i32,
    pub si_utime: u32,
    pub si_stime: u32,
    pub si_value: u64,
    pub _pad: [u32; 20],
    pub _align: [u64; 0],
}

#[allow(unused)]
impl SigInfo {
    /// sent by kill, sigsend, raise
    pub const USER: i32 = 0;
    /// sent by the kernel from somewhere
    pub const KERNEL: i32 = 0x80;
    /// sent by sigqueue
    pub const QUEUE: i32 = -1;
    /// sent by timer expiration
    pub const TIMER: i32 = -2;
    /// sent by real time mesq state change
    pub const MESGQ: i32 = -3;
    /// sent by AIO completion
    pub const ASYNCIO: i32 = -4;
    /// sent by queued SIGIO
    pub const SIGIO: i32 = -5;
    /// sent by tkill system call
    pub const TKILL: i32 = -6;
    /// sent by execve() killing subsidiary threads
    pub const DETHREAD: i32 = -7;
    /// sent by glibc async name lookup completion
    pub const ASYNCNL: i32 = -60;

    // SIGCHLD si_codes
    /// child has exited
    pub const CLD_EXITED: i32 = 1;
    /// child was killed
    pub const CLD_KILLED: i32 = 2;
    /// child terminated abnormally
    pub const CLD_DUMPED: i32 = 3;
    /// traced child has trapped
    pub const CLD_TRAPPED: i32 = 4;
    /// child has stopped
    pub const CLD_STOPPED: i32 = 5;
    /// stopped child has continued
    pub const CLD_CONTINUED: i32 = 6;
    pub const NSIGCHLD: i32 = 6;
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Sig(i32);

/// Sig为0时表示空信号，从1开始才是有含义的信号
#[allow(unused)]
impl Sig {
    pub const SIGHUP: Sig = Sig(1); // Hangup detected on controlling terminal or death of controlling process
    pub const SIGINT: Sig = Sig(2); // Interrupt from keyboard
    pub const SIGQUIT: Sig = Sig(3); // Quit from keyboard
    pub const SIGILL: Sig = Sig(4); // Illegal Instruction
    pub const SIGTRAP: Sig = Sig(5); // Trace/breakpoint trap
    pub const SIGABRT: Sig = Sig(6); // Abort signal from abort(3)
    pub const SIGBUS: Sig = Sig(7); // Bus error (bad memory access)
    pub const SIGFPE: Sig = Sig(8); // Floating point exception
    pub const SIGKILL: Sig = Sig(9); // Kill signal
    pub const SIGUSR1: Sig = Sig(10); // User-defined signal 1
    pub const SIGSEGV: Sig = Sig(11); // Invalid memory reference
    pub const SIGUSR2: Sig = Sig(12); // User-defined signal 2
    pub const SIGPIPE: Sig = Sig(13); // Broken pipe: write to pipe with no readers
    pub const SIGALRM: Sig = Sig(14); // Timer signal from alarm(2)
    pub const SIGTERM: Sig = Sig(15); // Termination signal
    pub const SIGSTKFLT: Sig = Sig(16); // Stack fault on coprocessor (unused)
    pub const SIGCHLD: Sig = Sig(17); // Child stopped or terminated
    pub const SIGCONT: Sig = Sig(18); // Continue if stopped
    pub const SIGSTOP: Sig = Sig(19); // Stop process
    pub const SIGTSTP: Sig = Sig(20); // Stop typed at terminal
    pub const SIGTTIN: Sig = Sig(21); // Terminal input for background process
    pub const SIGTTOU: Sig = Sig(22); // Terminal output for background process
    pub const SIGURG: Sig = Sig(23); // Urgent condition on socket (4.2BSD)
    pub const SIGXCPU: Sig = Sig(24); // CPU time limit exceeded (4.2BSD)
    pub const SIGXFSZ: Sig = Sig(25); // File size limit exceeded (4.2BSD)
    pub const SIGVTALRM: Sig = Sig(26); // Virtual alarm clock (4.2BSD)
    pub const SIGPROF: Sig = Sig(27); // Profiling alarm clock
    pub const SIGWINCH: Sig = Sig(28); // Window resize signal (4.3BSD, Sun)
    pub const SIGIO: Sig = Sig(29); // I/O now possible (4.2BSD)
    pub const SIGPWR: Sig = Sig(30); // Power failure (System V)
    pub const SIGSYS: Sig = Sig(31); // Bad system call (SVr4); unused on Linux
    pub const SIGLEGACYMAX: Sig = Sig(32); // Legacy maximum signal
    pub const SIGMAX: Sig = Sig(64); // Maximum signal

    /// Creates a new `Sig` from a signal number.
    ///
    /// `signum` must be a valid signal number, or 0 as a placeholder value.
    pub fn from_i32(signum: i32) -> Sig {
        debug_assert!(
            signum >= 0 && signum < NSIG as i32,
            "Invalid signal number: {}",
            signum
        );
        Sig(signum)
    }

    /// Returns true if the signal is a valid signal number.
    ///
    /// Note that 0 is not a valid signal number, but is often used as a placeholder.
    pub fn is_valid(&self) -> bool {
        self.0 > 0 && self.0 < NSIG as i32
    }

    pub fn raw(&self) -> usize {
        self.0 as usize
    }

    pub fn index(&self) -> usize {
        (self.0 - 1) as usize
    }
}

impl fmt::Display for Sig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Sig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", SigSet::from(*self))
    }
}

impl From<usize> for Sig {
    fn from(item: usize) -> Self {
        Sig(item as i32) // 这里假设usize到i32的转换是安全的，但要注意溢出的风险
    }
}

bitflags! {
    #[derive(Copy, Clone, Default, Debug)]
    pub struct SigSet: u64 {
        const SIGHUP    = 1 << 0 ;
        const SIGINT    = 1 << 1 ;
        const SIGQUIT   = 1 << 2 ;
        const SIGILL    = 1 << 3 ;
        const SIGTRAP   = 1 << 4 ;
        const SIGABRT   = 1 << 5 ;
        const SIGBUS    = 1 << 6 ;
        const SIGFPE    = 1 << 7 ;
        const SIGKILL   = 1 << 8 ;
        const SIGUSR1   = 1 << 9 ;
        const SIGSEGV   = 1 << 10;
        const SIGUSR2   = 1 << 11;
        const SIGPIPE   = 1 << 12;
        const SIGALRM   = 1 << 13;
        const SIGTERM   = 1 << 14;
        const SIGSTKFLT = 1 << 15;
        const SIGCHLD   = 1 << 16;
        const SIGCONT   = 1 << 17;
        const SIGSTOP   = 1 << 18;
        const SIGTSTP   = 1 << 19;
        const SIGTTIN   = 1 << 20;
        const SIGTTOU   = 1 << 21;
        const SIGURG    = 1 << 22;
        const SIGXCPU   = 1 << 23;
        const SIGXFSZ   = 1 << 24;
        const SIGVTALRM = 1 << 25;
        const SIGPROF   = 1 << 26;
        const SIGWINCH  = 1 << 27;
        const SIGIO     = 1 << 28;
        const SIGPWR    = 1 << 29;
        const SIGSYS    = 1 << 30;
        const SIGLEGACYMAX  = 1 << 31;

        // TODO: rt signal
        const SIGRT1    = 1 << (33 - 1);   // real time signal min
        const SIGRT2    = 1 << (34 - 1);
        const SIGRT3    = 1 << (35 - 1);
        const SIGRT4    = 1 << (36 - 1);
        const SIGRT5    = 1 << (37 - 1);
        const SIGRT6    = 1 << (38 - 1);
        const SIGRT7    = 1 << (39 - 1);
        const SIGRT8    = 1 << (40 - 1);
        const SIGRT9    = 1 << (41 - 1);
        const SIGRT10    = 1 << (42 - 1);
        const SIGRT11    = 1 << (43 - 1);
        const SIGRT12   = 1 << (44 - 1);
        const SIGRT13   = 1 << (45 - 1);
        const SIGRT14   = 1 << (46 - 1);
        const SIGRT15   = 1 << (47 - 1);
        const SIGRT16   = 1 << (48 - 1);
        const SIGRT17   = 1 << (49 - 1);
        const SIGRT18   = 1 << (50 - 1);
        const SIGRT19   = 1 << (51 - 1);
        const SIGRT20   = 1 << (52 - 1);
        const SIGRT21   = 1 << (53 - 1);
        const SIGRT22   = 1 << (54 - 1);
        const SIGRT23   = 1 << (55 - 1);
        const SIGRT24   = 1 << (56 - 1);
        const SIGRT25   = 1 << (57 - 1);
        const SIGRT26   = 1 << (58 - 1);
        const SIGRT27   = 1 << (59 - 1);
        const SIGRT28   = 1 << (60 - 1);
        const SIGRT29   = 1 << (61 - 1);
        const SIGRT30   = 1 << (62 - 1);
        const SIGRT31   = 1 << (63 - 1);

        const SIGMAX   = 1 << 63;

        // 下面信号通常是由程序中的错误或异常操作触发的，如非法内存访问（导致
        // SIGSEGV）、硬件异常（可能导致
        // SIGBUS）等。同步信号的处理通常需要立即响应，
        // 因为它们指示了程序运行中的严重问题
        const SYNCHRONOUS_MASK = SigSet::SIGSEGV.bits()
            | SigSet::SIGBUS.bits()
            | SigSet::SIGILL.bits()
            | SigSet::SIGTRAP.bits()
            | SigSet::SIGFPE.bits()
            | SigSet::SIGSYS.bits();

        const DUMP_MASK = SigSet::SIGABRT.bits()
            | SigSet::SIGBUS.bits()
            | SigSet::SIGFPE.bits()
            | SigSet::SIGILL.bits()
            | SigSet::SIGQUIT.bits()
            | SigSet::SIGSEGV.bits()
            | SigSet::SIGSYS.bits()
            | SigSet::SIGTRAP.bits()
            | SigSet::SIGXCPU.bits()
            | SigSet::SIGXFSZ.bits();
    }
}

impl SigSet {
    pub fn add_signal(&mut self, sig: Sig) {
        self.insert(SigSet::from_bits(1 << sig.index()).unwrap())
    }

    pub fn contain_signal(&self, sig: Sig) -> bool {
        self.contains(SigSet::from_bits(1 << sig.index()).unwrap())
    }

    pub fn remove_signal(&mut self, sig: Sig) {
        self.remove(SigSet::from_bits(1 << sig.index()).unwrap())
    }

    /// 从buf中读取（不足补0，超长丢弃），构造成Kernel内部的SigSet
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut raw = [0u8; core::mem::size_of::<SigSet>()];
        let to_copy = core::cmp::min(bytes.len(), raw.len());
        raw[..to_copy].copy_from_slice(&bytes[..to_copy]);
        unsafe { core::ptr::read(raw.as_ptr() as *const Self) }
    }

    /// 按要求把mask内容写到指定buf（多余buf位置不写）
    pub fn write_bytes(&self, out: &mut [u8]) {
        let src = unsafe {
            core::slice::from_raw_parts(
                (self as *const SigSet) as *const u8,
                core::mem::size_of::<SigSet>(),
            )
        };
        let to_copy = core::cmp::min(out.len(), src.len());
        out[..to_copy].copy_from_slice(&src[..to_copy]);
        // out多余部分用户态已清零，正常无需写
    }
}

impl From<Sig> for SigSet {
    fn from(sig: Sig) -> Self {
        Self::from_bits(1 << sig.index()).unwrap()
    }
}
