mod consts;
mod fs;
mod misc;
mod mm;
mod net;
mod process;
mod sche;
mod signal;
mod time;
mod user;

use consts::SyscallNo::{self, *};
use driver::println;
use fs::*;
use misc::{sys_getrandom, sys_sysinfo, sys_syslog, sys_uname};
use mm::*;
use net::*;
use process::*;
use sche::*;
use signal::*;
use time::{
    sys_clock_gettime, sys_clock_nanosleep, sys_getitimer, sys_gettimeofday, sys_nanosleep,
    sys_setitimer, sys_times,
};
use user::{sys_getgid, sys_getuid};

use crate::syscall::time::sys_clock_getres;

pub async fn syscall(syscall_no: usize, args: [usize; 6]) -> usize {
    let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
        log::error!("Syscall number not included: {syscall_no}");
        unimplemented!()
    };

    if 1 == 0 {
        log::warn!(
            "task {} call [{}]",
            crate::processor::current_task().tid(),
            syscall_no.as_str(),
        );
    }

    let result = match syscall_no {
        GETTIMEOFDAY => sys_gettimeofday(args[0], args[1]).await,
        EXIT => sys_exit(args[0] as i32),
        EXIT_GROUP => sys_exit_group(args[0] as i32),
        SCHED_YIELD => sys_sched_yield().await,
        WRITE => sys_write(args[0], args[1], args[2]).await,
        TIMES => sys_times(args[0]).await,
        NANOSLEEP => sys_nanosleep(args[0], args[1]).await,
        WAIT4 => sys_wait4(args[0] as i32, args[1], args[2] as i32).await,
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        OPENAT => sys_openat(args[0], args[1], args[2] as i32, args[3] as u32).await,
        READ => sys_read(args[0], args[1], args[2]).await,
        READLINKAT => sys_readlinkat(args[0], args[1], args[2], args[3]),
        LSEEK => sys_lseek(args[0], args[1] as isize, args[2]),
        EXECVE => sys_execve(args[0], args[1], args[2]).await,
        GETPID => sys_getpid(),
        GETTID => sys_gettid(),
        GETCWD => sys_getcwd(args[0], args[1]).await,
        FSTAT => sys_fstat(args[0], args[1]),
        FSTATAT => sys_fstatat(args[0], args[1], args[2], args[3] as i32),
        CLOSE => sys_close(args[0]),
        GETPPID => sys_getppid(),
        UNAME => sys_uname(args[0]).await,
        DUP => sys_dup(args[0]),
        DUP3 => sys_dup3(args[0], args[1], args[2] as i32),
        MMAP => {
            sys_mmap(
                args[0],
                args[1],
                args[2] as i32,
                args[3] as i32,
                args[4] as isize,
                args[5],
            )
            .await
        }
        MKDIR => sys_mkdirat(args[0], args[1], args[2] as u32).await,
        CHDIR => sys_chdir(args[0]).await,
        BRK => sys_brk(args[0]).await,
        UNLINKAT => sys_unlinkat(args[0], args[1], args[2] as i32).await,
        GETDENTS64 => sys_getdents64(args[0], args[1], args[2]).await,
        MOUNT => sys_mount(args[0], args[1], args[2], args[3] as u32, args[4]).await,
        FACCESSAT => sys_faccessat(args[0], args[1], args[2] as i32).await,
        SET_TID_ADDRESS => sys_set_tid_address(args[0]),
        SET_ROBUST_LIST => sys_set_robust_list(args[0], args[1]),
        GET_ROBUST_LIST => sys_get_robust_list(args[0] as i32, args[1], args[2]),
        UMOUNT2 => sys_umount2(args[0], args[1] as u32).await,
        MUNMAP => sys_munmap(args[0], args[1]).await,
        PIPE2 => sys_pipe2(args[0], args[1] as i32).await,
        MPROTECT => sys_mprotect(args[0], args[1], args[2] as i32),
        GETUID => sys_getuid(),
        GETGID => sys_getgid(),
        IOCTL => sys_ioctl(args[0], args[1], args[2]),
        SETGID => sys_setgid(args[0]),
        SETUID => sys_setuid(args[0]),
        CLOCK_GETTIME => sys_clock_gettime(args[0], args[1]),
        SENDFILE => sys_sendfile64(args[0], args[1], args[2], args[3]).await,
        RT_SIGACTION => sys_rt_sigaction(args[0] as i32, args[1], args[2], args[3]),
        FCNTL => sys_fcntl(args[0], args[1] as isize, args[2]),
        WRITEV => sys_writev(args[0], args[1], args[2]).await,
        READV => sys_readv(args[0], args[1], args[2]).await,
        RT_SIGPROCMASK => sys_rt_sigmask(args[0], args[1], args[2], args[3]),
        RT_SIGRETURN => sys_sigreturn().await,
        TGKILL => sys_tgkill(args[0] as isize, args[1] as isize, args[2] as i32),
        GETPGID => sys_getpgid(args[0]),
        SETPGID => sys_setpgid(args[0], args[1]),
        GETEUID => sys_geteuid(),
        PPOLL => sys_ppoll(args[0], args[1], args[2], args[3]).await,
        STATFS => sys_statfs(args[0], args[1]),
        SYSLOG => sys_syslog(args[0], args[1], args[2]),
        SYSINFO => sys_sysinfo(args[0]),
        KILL => sys_kill(args[0] as isize, args[1] as i32),
        CLOCK_NANOSLEEP => sys_clock_nanosleep(args[0], args[1], args[2], args[3]).await,
        UTIMENSAT => sys_utimensat(args[0], args[1], args[2], args[3] as i32),
        RENAMEAT2 => sys_renameat2(args[0], args[1], args[2], args[3], args[4] as i32),
        LINKAT => sys_linkat(args[0], args[1], args[2], args[3], args[4] as i32),
        SYMLINKAT => sys_symlinkat(args[0], args[1], args[2]),
        SYNC => sys_sync(),
        SETITIMER => sys_setitimer(args[0], args[1], args[2]),
        GETITIMER => sys_getitimer(args[0], args[1]),
        UMASK => sys_umask(args[0] as i32),
        PRLIMIT64 => sys_prlimit64(args[0], args[1] as i32, args[2], args[3]),
        GETRANDOM => sys_getrandom(args[0], args[1], args[2] as i32),
        RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(args[0], args[1], args[2]).await,
        FTRUNCATE => sys_ftruncate(args[0], args[1]),
        FSYNC => sys_fsync(args[0]),
        FUTEX => {
            sys_futex(
                args[0],
                args[1] as _,
                args[2] as u32,
                args[3],
                args[4],
                args[5] as u32,
            )
            .await
        }
        MADVISE => sys_madvise(args[0], args[1], args[2]),
        SHMGET => sys_shmget(args[0], args[1], args[2] as i32),
        TKILL => sys_tkill(args[0] as isize, args[1] as i32),
        SOCKET => sys_socket(args[0], args[1] as i32, args[2]),
        BIND => sys_bind(args[0], args[1], args[2]),
        GETSOCKNAME => sys_getsockname(args[0], args[1], args[2]),
        SHMAT => sys_shmat(args[0], args[1], args[2] as i32),
        SHMDT => sys_shmdt(args[0]),
        SHMCTL => sys_shmctl(args[0], args[1] as i32, args[2]),
        PSELECT6 => sys_pselect6(args[0] as i32, args[1], args[2], args[3], args[4], args[5]).await,
        PREAD64 => sys_pread64(args[0], args[1], args[2], args[3]).await,
        PWRITE64 => sys_pwrite64(args[0], args[1], args[2], args[3]).await,
        SETSOCKOPT => sys_setsockopt(args[0], args[1], args[2], args[3], args[4]),
        GETSOCKOPT => sys_getsockopt(args[0], args[1], args[2], args[3], args[4]),
        SENDTO => sys_sendto(args[0], args[1], args[2], args[3], args[4], args[5]).await,
        RECVFROM => sys_recvfrom(args[0], args[1], args[2], args[3], args[4], args[5]).await,
        LISTEN => sys_listen(args[0], args[1]),
        CONNECT => sys_connect(args[0], args[1], args[2]).await,
        ACCEPT => sys_accept(args[0], args[1], args[2]).await,
        GETEGID => sys_getegid(),
        MEMBARRIER => sys_membarrier(args[0], args[1], args[2]),
        SHUTDOWN => sys_shutdown(args[0], args[1]),
        STATX => sys_statx(args[0], args[1], args[2], args[3], args[4]),
        CLONE3 => sys_clone3(args[0], args[1]),
        MREMAP => sys_mremap(args[0], args[1], args[2], args[3] as i32, args[4]),
        SETSID => sys_setsid(),
        SCHED_GETAFFINITY => sys_sched_getaffinity(args[0], args[1], args[2]),
        GETMEMPOLICY => sys_get_mempolicy(args[0], args[1], args[2], args[3], args[4] as isize),
        SCHED_GETSCHEDULER => sys_sched_getscheduler(),
        SCHED_GETPARAM => sys_sched_getparam(),
        SCHED_SETSCHEDULER => sys_sched_setscheduler(),
        CLOCK_GETRES => sys_clock_getres(args[0], args[1]),
        MLOCK => sys_mlock(args[0], args[1]),
        MUNLOCK => sys_munlock(args[0], args[1]),
        SCHED_SETAFFINITY => sys_sched_setaffinity(args[0], args[1], args[2]),
        SOCKETPAIR => sys_socketpair(args[0], args[1], args[2], args[3]),
        GETRUSAGE => sys_getrusage(args[0] as i32, args[1]),
        GETPEERNAME => sys_getpeername(args[0], args[1], args[2]),
        _ => {
            println!("Syscall not implemented: {}", syscall_no.as_str());
            unimplemented!()
        }
    };

    match result {
        Ok(ret) => ret,
        Err(e) => {
            log::warn!("[syscall] {syscall_no} return err {e:?}");
            -(e as isize) as usize
        }
    }
}
