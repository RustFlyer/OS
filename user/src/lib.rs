#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;

mod error;
mod lang_items;
#[allow(unused)]
mod syscall;

extern crate alloc;

use alloc::{ffi::CString, vec::Vec};

use buddy_system_allocator::LockedHeap;
use config::{inode::InodeMode, vfs::OpenFlags};
pub use error::SyscallErr;
use sig::{Sig, SigAction};
use strum::FromRepr;
use syscall::*;

// const USER_HEAP_SIZE: usize = 16384;
const USER_HEAP_SIZE: usize = 0x32000;

// Note that heap space is allocated in .data segment
// TODO: can we change to dynamically allocate by invoking sys_sbrk?
static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[allow(static_mut_refs)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);

        // FIXME: heap alloc will meet trouble when triple fork
        // const HEAP_START: usize = 0x0000_0002_0000_0000;
        // sys_brk(HEAP_START + USER_HEAP_SIZE);
        // HEAP.lock().init(HEAP_START, USER_HEAP_SIZE);
    }
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        let len = (0usize..)
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    let exit_code = main(argc, v.as_slice());
    // println!("program {} will exit", v[0]);
    exit(exit_code);
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main(_: usize, _: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

#[macro_export]
macro_rules! wexitstatus {
    ($a:expr) => {
        ($a & 0xffffff00) >> 8
    };
}

// pub fn getcwd(path: usize, len: usize) -> isize {
//     sys_getcwd(path, len)
// }

// pub fn mount(dev_name: usize, target_path: usize, ftype: usize, flags: u32,
// data: usize) -> isize {     sys_mount(dev_name, target_path, ftype, flags,
// data) }

// pub fn uname(buf: usize) -> isize {
//     sys_uname(buf)
// }

//************file system***************/
pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}

pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf.as_mut_ptr(), buf.len())
}
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf.as_ptr(), buf.len())
}
pub fn mmap(
    addr: *const u8,
    length: usize,
    prot: i32,
    flags: i32,
    fd: usize,
    offset: usize,
) -> isize {
    sys_mmap(
        addr as usize,
        length,
        prot as usize,
        flags as usize,
        fd,
        offset,
    )
}

pub fn open(dirfd: i32, pathname: &str, flags: OpenFlags, mode: InodeMode) -> isize {
    let pathname = CString::new(pathname).unwrap();
    sys_openat(
        dirfd as usize,
        pathname.as_ptr() as _,
        flags.bits() as usize,
        mode.bits() as usize,
    )
}

pub fn lseek(fd: usize, offset: isize, whence: usize) -> isize {
    sys_lseek(fd, offset, whence)
}

pub fn getdents(fd: usize, buf: &mut [u8], len: usize) -> isize {
    sys_getdents64(fd, buf.as_mut_ptr(), len)
}

//************ task ***************/
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code);
    loop {}
}
pub fn exit_group(exit_code: i32) -> ! {
    sys_exit_group(exit_code);
    loop {}
}
pub fn yield_() -> isize {
    sys_yield()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn prlimit64(pid: usize, resource: i32, new_limit: usize, old_limit: &RLimit) -> isize {
    sys_prlimit64(
        pid,
        resource as usize,
        new_limit,
        old_limit as *const RLimit as usize,
    )
}

pub fn clone(
    flags: usize,
    stack: usize,
    parent_tid_ptr: usize,
    tls_ptr: usize,
    chilren_tid_ptr: usize,
) -> isize {
    sys_clone(flags, stack, parent_tid_ptr, tls_ptr, chilren_tid_ptr)
}

pub fn kill(pid: isize, sig: usize) -> isize {
    sys_kill(pid as usize, sig as i32)
}
pub fn execve(path: &str, argv: &[&str], envp: &[&str]) -> isize {
    let path = CString::new(path).unwrap();
    let argv: Vec<_> = argv.iter().map(|s| CString::new(*s).unwrap()).collect();
    let envp: Vec<_> = envp.iter().map(|s| CString::new(*s).unwrap()).collect();
    let mut argv = argv.iter().map(|s| s.as_ptr() as usize).collect::<Vec<_>>();
    let mut envp = envp.iter().map(|s| s.as_ptr() as usize).collect::<Vec<_>>();
    argv.push(0);
    envp.push(0);
    sys_execve(path.as_ptr() as *const u8, argv.as_ptr(), envp.as_ptr())
}

pub fn wait(exit_code: &mut i32) -> isize {
    sys_waitpid(-1, exit_code as *mut _, 0)
}

pub fn waitpid(pid: isize, exit_code: &mut i32) -> isize {
    sys_waitpid(pid, exit_code as *mut _, 0)
}

pub fn pipe(pipe_fd: &mut [i32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr())
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}

//************ time ***************/
pub struct TimeSpec {
    _tv_sec: usize,
    _tv_nsec: usize,
}
impl TimeSpec {
    pub fn into_ms(&self) -> usize {
        self._tv_sec * 1_000 + self._tv_nsec / 1_000_000
    }

    pub fn from_ms(ms: usize) -> Self {
        Self {
            _tv_sec: ms / 1000,
            _tv_nsec: (ms % 1000) * 1_000_000,
        }
    }

    pub fn is_valid(&self) -> bool {
        (self._tv_sec as isize > 0)
            && (self._tv_nsec as isize > 0)
            && (self._tv_nsec < 1_000_000_000)
    }
}

#[derive(Debug)]
pub struct TimeVal {
    _tv_sec: usize,
    _tv_usec: usize,
}

pub fn gettimeofday(time_val: &mut TimeVal) -> isize {
    sys_gettimeofday(time_val as *mut TimeVal as *mut usize, 0 as *mut usize)
}

pub fn nanosleep(req: &TimeSpec, rem: &mut TimeSpec) -> isize {
    sys_nanosleep(
        req as *const TimeSpec as *const usize,
        rem as *mut TimeSpec as *mut usize,
    )
}

pub fn sleep(ms: usize) -> isize {
    let req = TimeSpec::from_ms(ms);
    let mut rem = TimeSpec::from_ms(0);
    nanosleep(&req, &mut rem)
}

//************ signal ***************/
pub fn sigaction(sig_no: Sig, act: &SigAction, old_act: &mut SigAction) -> isize {
    sys_sigaction(
        sig_no.raw(),
        act as *const SigAction as *const usize,
        old_act as *mut SigAction as *mut usize,
    )
}

pub fn sigreturn() -> isize {
    sys_sigreturn()
}

pub mod sig {
    use core::fmt;

    use bitflags::bitflags;

    #[derive(Clone, Copy, Default)]
    #[repr(C)]
    pub struct SigAction {
        /// sa_handler specifies the action to be associated with signum and can be
        /// one of the following:
        /// 1. SIG_DFL for the default action
        /// 2. SIG_IGN to ignore this signal
        /// 3. A pointer to a signal handling function. This function receives the
        ///    signal number as its only argument.
        pub sa_handler: usize,
        pub sa_flags: SigActionFlag,
        pub restorer: usize,
        /// sa_mask specifies a mask of signals which should be blocked during
        /// execution of the signal handler.
        pub sa_mask: SigSet,
    }

    bitflags! {
        #[derive(Default, Copy, Clone)]
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

    bitflags! {
        pub struct OpenFlags: u32 {
            const O_RDONLY = 0;
            const O_WRONLY = 1 << 0;
            const O_RDWR = 1 << 1;
            const O_CLOEXEC = 1 << 7;
            const O_CREATE = 1 << 9;
            const O_TRUNC = 1 << 10;
        }
    }

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

    #[derive(Clone, Copy, Debug)]
    #[repr(C)]
    pub struct UContext {
        pub uc_flags: usize,
        /// 当前上下文返回时将恢复执行的下一个上下文的指针
        pub uc_link: usize,
        // 当前上下文使用的栈信息,包含栈的基址、大小等信息
        pub uc_stack: SignalStack,
        // 当前上下文活跃时被阻塞的信号集
        pub uc_sigmask: SigSet,
        // 保存具体机器状态的上下文信息，这是一个机器相关的表示，包含了处理器的寄存器状态等信息
        pub uc_mcontext: MContext,
    }

    #[derive(Clone, Copy, Debug)]
    #[repr(C)]
    pub struct MContext {
        pub sepc: usize,
        pub user_x: [usize; 32],
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Sig(i32);

    /// Sig为0时表示空信号，从1开始才是有含义的信号
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

        pub fn from_i32(signum: i32) -> Sig {
            Sig(signum as i32)
        }

        pub fn is_valid(&self) -> bool {
            self.0 >= 0 && self.0 < 1024 as i32
        }

        pub fn raw(&self) -> usize {
            self.0 as usize
        }

        pub fn index(&self) -> usize {
            (self.0 - 1) as usize
        }

        pub fn is_kill_or_stop(&self) -> bool {
            matches!(*self, Sig::SIGKILL | Sig::SIGSTOP)
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
            const SYNCHRONOUS_MASK = SigSet::SIGSEGV.bits() | SigSet::SIGBUS.bits()
            | SigSet::SIGILL.bits() | SigSet::SIGTRAP.bits() | SigSet::SIGFPE.bits() | SigSet::SIGSYS.bits();
            // const SYNCHRONOUS_MASK = (1<<3) | (1<<4) | (1<<6) | (1<<7) | (1<<10) | (1<<30) ;
        }
    }

    impl From<Sig> for SigSet {
        fn from(sig: Sig) -> Self {
            Self::from_bits(1 << sig.index()).unwrap()
        }
    }

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
        },
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
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    /// Soft limit: the kernel enforces for the corresponding resource
    pub rlim_cur: usize,
    /// Hard limit (ceiling for rlim_cur)
    pub rlim_max: usize,
}

#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Resource {
    // Per-process CPU limit, in seconds.
    CPU = 0,
    // Largest file that can be created, in bytes.
    FSIZE = 1,
    // Maximum size of data segment, in bytes.
    DATA = 2,
    // Maximum size of stack segment, in bytes.
    STACK = 3,
    // Largest core file that can be created, in bytes.
    CORE = 4,
    // Largest resident set size, in bytes.
    // This affects swapping; processes that are exceeding their
    // resident set size will be more likely to have physical memory
    // taken from them.
    RSS = 5,
    // Number of processes.
    NPROC = 6,
    // Number of open files.
    NOFILE = 7,
    // Locked-in-memory address space.
    MEMLOCK = 8,
    // Address space limit.
    AS = 9,
    // Maximum number of file locks.
    LOCKS = 10,
    // Maximum number of pending signals.
    SIGPENDING = 11,
    // Maximum bytes in POSIX message queues.
    MSGQUEUE = 12,
    // Maximum nice priority allowed to raise to.
    // Nice levels 19 .. -20 correspond to 0 .. 39
    // values of this resource limit.
    NICE = 13,
    // Maximum realtime priority allowed for non-priviledged
    // processes.
    RTPRIO = 14,
    // Maximum CPU time in microseconds that a process scheduled under a real-time
    // scheduling policy may consume without making a blocking system
    // call before being forcibly descheduled.
    RTTIME = 15,
}
