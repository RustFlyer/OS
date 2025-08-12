use core::arch::asm;

const SYSCALL_GETCWD: usize = 17;
const SYSCALL_DUP: usize = 23;
const SYSCALL_DUP3: usize = 24;
const SYSCALL_FCNTL: usize = 25;
const SYSCALL_IOCTL: usize = 29;
const SYSCALL_UNLINK: usize = 35;
const SYSCALL_MKNOD: usize = 33;
const SYSCALL_MKDIR: usize = 34;
const SYSCALL_UMOUNT: usize = 39;
const SYSCALL_MOUNT: usize = 40;
const SYSCALL_STATFS: usize = 43;
const SYSCALL_FTRUNCATE: usize = 46;
const SYSCALL_FACCESSAT: usize = 48;
const SYSCALL_CHDIR: usize = 49;
const SYSCALL_FCHMODAT: usize = 53;
const SYSCALL_OPEN: usize = 56;
const SYSCALL_CLOSE: usize = 57;
const SYSCALL_PIPE: usize = 59;
const SYSCALL_GETDENTS: usize = 61;
const SYSCALL_LSEEK: usize = 62;
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_READV: usize = 65;
const SYSCALL_WRITEV: usize = 66;
const SYSCALL_PREAD64: usize = 67;
const SYSCALL_PWRITE64: usize = 68;
const SYSCALL_SENDFILE: usize = 71;
const SYSCALL_PSELECT6: usize = 72;
const SYSCALL_PPOLL: usize = 73;
const SYSCALL_READLINKAT: usize = 78;
const SYSCALL_NEWFSTATAT: usize = 79;
const SYSCALL_FSTAT: usize = 80;
const SYSCALL_SYNC: usize = 81;
const SYSCALL_FSYNC: usize = 82;
const SYSCALL_UTIMENSAT: usize = 88;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_EXIT_GROUP: usize = 94;
const SYSCALL_WAITID: usize = 95;
const SYSCALL_SET_TID_ADDRESS: usize = 96;
const SYSCALL_FUTEX: usize = 98;
const SYSCALL_SET_ROBUST_LIST: usize = 99;
const SYSCALL_GET_ROBUST_LIST: usize = 100;
const SYSCALL_NANOSLEEP: usize = 101;
const SYSCALL_GETITIMER: usize = 102;
const SYSCALL_SETITIMER: usize = 103;
const SYSCALL_CLOCK_SETTIME: usize = 112;
const SYSCALL_CLOCK_GETTIME: usize = 113;
const SYSCALL_CLOCK_GETRES: usize = 114;
const SYSCALL_CLOCK_NANOSLEEP: usize = 115;
const SYSCALL_SYSLOG: usize = 116;
const SYSCALL_SCHED_SETSCHEDULER: usize = 119;
const SYSCALL_SCHED_GETSCHEDULER: usize = 120;
const SYSCALL_SCHED_GETPARAM: usize = 121;
const SYSCALL_SCHED_SETAFFINITY: usize = 122;
const SYSCALL_SCHED_GETAFFINITY: usize = 123;
const SYSCALL_SCHED_YIELD: usize = 124;
const SYSCALL_KILL: usize = 129;
const SYSCALL_TKILL: usize = 130;
const SYSCALL_TGKILL: usize = 131;
const SYSCALL_RT_SIGSUSPEND: usize = 133;
const SYSCALL_RT_SIGACTION: usize = 134;
const SYSCALL_RT_SIGPROCMASK: usize = 135;
const SYSCALL_RT_SIGTIMEDWAIT: usize = 137;
const SYSCALL_RT_SIGRETURN: usize = 139;
const SYSCALL_SETUID: usize = 146;
const SYSCALL_TIMES: usize = 153;
const SYSCALL_SETPGID: usize = 154;
const SYSCALL_GETPGID: usize = 155;
const SYSCALL_SETSID: usize = 157;
const SYSCALL_UNAME: usize = 160;
const SYSCALL_GETRUSAGE: usize = 165;
const SYSCALL_UMASK: usize = 166;
const SYSCALL_GETTIMEOFDAY: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_GETPPID: usize = 173;
const SYSCALL_GETUID: usize = 174;
const SYSCALL_GETEUID: usize = 175;
const SYSCALL_GETGID: usize = 176;
const SYSCALL_GETEGID: usize = 177;
const SYSCALL_GETTID: usize = 178;
const SYSCALL_SYSINFO: usize = 179;
const SYSCALL_SHMGET: usize = 194;
const SYSCALL_SHMCTL: usize = 195;
const SYSCALL_SHMAT: usize = 196;
const SYSCALL_SOCKET: usize = 198;
const SYSCALL_SOCKETPAIR: usize = 199;
const SYSCALL_BIND: usize = 200;
const SYSCALL_LISTEN: usize = 201;
const SYSCALL_ACCEPT: usize = 202;
const SYSCALL_CONNECT: usize = 203;
const SYSCALL_GETSOCKNAME: usize = 204;
const SYSCALL_GETPEERNAME: usize = 205;
const SYSCALL_SENDTO: usize = 206;
const SYSCALL_RECVFROM: usize = 207;
const SYSCALL_SETSOCKOPT: usize = 208;
const SYSCALL_GETSOCKOPT: usize = 209;
const SYSCALL_SHUTDOWN: usize = 210;
const SYSCALL_BRK: usize = 214;
const SYSCALL_MUNMAP: usize = 215;
const SYSCALL_CLONE: usize = 220;
const SYSCALL_EXECVE: usize = 221;
const SYSCALL_MMAP: usize = 222;
const SYSCALL_MPROTECT: usize = 226;
const SYSCALL_MSYNC: usize = 227;
const SYSCALL_MADVISE: usize = 233;
const SYSCALL_WAIT4: usize = 260;
const SYSCALL_PRLIMIT64: usize = 261;
const SYSCALL_REMANEAT2: usize = 276;
const SYSCALL_GETRANDOM: usize = 278;
const SYSCALL_MEMBARRIER: usize = 283;
const SYSCALL_COPY_FILE_RANGE: usize = 285;

#[macro_export]
macro_rules! syscall {
    ($name:ident, $id:expr) => {
        pub fn $name() -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty) => {
        pub fn $name(a0: $t0) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty) => {
        pub fn $name(a0: $t0, a1: $t1) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty, $t2:ty) => {
        pub fn $name(a0: $t0, a1: $t1, a2: $t2) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            args[2] = a2 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty, $t2:ty, $t3:ty) => {
        pub fn $name(a0: $t0, a1: $t1, a2: $t2, a3: $t3) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            args[2] = a2 as usize;
            args[3] = a3 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty) => {
        pub fn $name(a0: $t0, a1: $t1, a2: $t2, a3: $t3, a4: $t4) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            args[2] = a2 as usize;
            args[3] = a3 as usize;
            args[4] = a4 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty) => {
        pub fn $name(a0: $t0, a1: $t1, a2: $t2, a3: $t3, a4: $t4, a5: $t5) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 6] = [0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            args[2] = a2 as usize;
            args[3] = a3 as usize;
            args[4] = a4 as usize;
            args[5] = a5 as usize;
            syscall($id, args)
        }
    };
    ($name:ident, $id:expr, $t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty, $t6:ty) => {
        pub fn $name(a0: $t0, a1: $t1, a2: $t2, a3: $t3, a4: $t4, a5: $t5) -> isize {
            use core::mem::MaybeUninit;

            #[cfg(target_arch = "riscv64")]
            compile_error!("syscall with 7 arguments is not supported on riscv64");

            #[cfg(target_arch = "riscv64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];
            #[cfg(target_arch = "loongarch64")]
            let mut args: [usize; 7] = [0, 0, 0, 0, 0, 0, 0];

            args[0] = a0 as usize;
            args[1] = a1 as usize;
            args[2] = a2 as usize;
            args[3] = a3 as usize;
            args[4] = a4 as usize;
            args[5] = a5 as usize;
            args[6] = a6 as usize;
            syscall($id, args)
        }
    };
}

#[cfg(target_arch = "riscv64")]
fn syscall(id: usize, args: [usize; 6]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") args[0] => ret,
            in("a1") args[1],
            in("a2") args[2],
            in("a3") args[3],
            in("a4") args[4],
            in("a5") args[5],
            in("a7") id
        );
    }
    ret
}

#[cfg(target_arch = "loongarch64")]
fn syscall(id: usize, args: [usize; 7]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "syscall 0",
            inlateout("$a0") args[0] => ret,
            in("$a1") args[1],
            in("$a2") args[2],
            in("$a3") args[3],
            in("$a4") args[4],
            in("$a5") args[5],
            in("$a6") args[6],
            in("$a7") id
        );
    }
    ret
}

syscall!(
    sys_mount,
    SYSCALL_MOUNT,
    *const u8,
    *const u8,
    *const u8,
    usize,
    *const u8
);

// file system
syscall!(sys_close, SYSCALL_CLOSE, usize);
syscall!(sys_getcwd, SYSCALL_GETCWD, *mut u8, usize);
syscall!(sys_chdir, SYSCALL_CHDIR, *const u8);
syscall!(sys_mkdir, SYSCALL_MKDIR, isize, *const u8, usize);
syscall!(sys_uname, SYSCALL_UNAME, *mut usize);
syscall!(sys_dup, SYSCALL_DUP, usize);
syscall!(sys_dup3, SYSCALL_DUP3, usize, usize, usize);
syscall!(sys_read, SYSCALL_READ, usize, *mut u8, usize);
syscall!(sys_write, SYSCALL_WRITE, usize, *const u8, usize);
syscall!(
    sys_mmap,
    SYSCALL_MMAP,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize
);
syscall!(sys_openat, SYSCALL_OPEN, usize, *const u8, usize, usize);
syscall!(sys_lseek, SYSCALL_LSEEK, usize, isize, usize);
syscall!(sys_getdents64, SYSCALL_GETDENTS, usize, *const u8, usize);

// task
syscall!(sys_getpid, SYSCALL_GETPID);
syscall!(sys_exit, SYSCALL_EXIT, i32);
syscall!(sys_exit_group, SYSCALL_EXIT_GROUP, i32);
syscall!(sys_waitid, SYSCALL_WAITID, i32, i32, *mut usize, i32);
syscall!(sys_kill, SYSCALL_KILL, usize, i32);
syscall!(sys_fork, SYSCALL_CLONE);
syscall!(sys_clone, SYSCALL_CLONE, usize, usize, usize, usize, usize);
syscall!(sys_waitpid, SYSCALL_WAIT4, isize, *mut i32, isize);
syscall!(sys_pipe, SYSCALL_PIPE, *mut i32);
syscall!(sys_brk, SYSCALL_BRK, usize);
syscall!(sys_yield, SYSCALL_SCHED_YIELD);
syscall!(sys_prlimit64, SYSCALL_PRLIMIT64, usize, usize, usize, usize);
syscall!(
    sys_execve,
    SYSCALL_EXECVE,
    *const u8,
    *const usize,
    *const usize
);

// signal
syscall!(
    sys_sigaction,
    SYSCALL_RT_SIGACTION,
    usize,
    *const usize,
    *mut usize
);
syscall!(sys_sigreturn, SYSCALL_RT_SIGRETURN);
syscall!(
    sys_sigmask,
    SYSCALL_RT_SIGPROCMASK,
    usize,
    usize,
    *mut usize
);

// Time
syscall!(
    sys_gettimeofday,
    SYSCALL_GETTIMEOFDAY,
    *mut usize,
    *mut usize
);
syscall!(sys_nanosleep, SYSCALL_NANOSLEEP, *const usize, *mut usize);
syscall!(sys_sleep, SYSCALL_NANOSLEEP, *const usize);
syscall!(sys_setuid, SYSCALL_SETUID, usize);
