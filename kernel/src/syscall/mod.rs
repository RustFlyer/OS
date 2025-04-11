mod consts;
mod fs;
mod misc;
mod mm;
mod process;
mod time;

use consts::SyscallNo::{self, *};
use fs::*;
use misc::sys_uname;
use mm::*;
use process::*;
use time::*;

pub async fn syscall(syscall_no: usize, args: [usize; 6]) -> usize {
    let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
        log::error!("Syscall number not included: {syscall_no}");
        unimplemented!()
    };

    let result = match syscall_no {
        GETTIMEOFDAY => sys_gettimeofday(args[0], args[1]),
        EXIT => sys_exit(args[0] as i32),
        SCHED_YIELD => sys_sched_yield().await,
        WRITE => sys_write(args[0], args[1], args[2]),
        TIMES => sys_times(args[0]),
        NANOSLEEP => sys_nanosleep(args[0], args[1]).await,
        WAIT4 => sys_waitpid().await,
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        OPENAT => sys_openat(args[0], args[1], args[2] as i32, args[3] as u32).await,
        READ => sys_read(args[0], args[1], args[2]),
        LSEEK => sys_lseek(args[0], args[1] as isize, args[2]),
        EXECVE => sys_execve(args[0], args[1], args[2]),
        GETPID => sys_getpid(),
        GETTID => sys_gettid(),
        GETCWD => sys_getcwd(args[0], args[1]),
        FSTAT => sys_fstat(args[0], args[1]),
        CLOSE => sys_close(args[0]),
        GETPPID => sys_getppid(),
        UNAME => sys_uname(args[0]),
        DUP => sys_dup(args[0]),
        DUP3 => sys_dup3(args[0], args[1], args[2] as i32),
        MMAP => sys_mmap(
            args[0],
            args[1],
            args[2] as i32,
            args[3] as i32,
            args[4],
            args[5],
        ),
        MKDIR => sys_mkdirat(args[0], args[1], args[2] as u32),
        CHDIR => sys_chdir(args[0]),
        _ => {
            log::error!("Syscall not implemented: {syscall_no}");
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
