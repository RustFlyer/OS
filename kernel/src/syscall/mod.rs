mod consts;
mod fs;
mod process;
mod signal;
mod time;

use consts::SyscallNo::{self, *};
use fs::*;
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
        WAIT4 => sys_wait4(args[0] as i32, args[1], args[2] as i32).await,
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        OPENAT => sys_openat(args[0], args[1], args[2] as i32, args[3] as u32).await,
        READ => sys_read(args[0], args[1], args[2]),
        LSEEK => sys_lseek(args[0], args[1] as isize, args[2]),
        EXECVE => sys_execve(args[0], args[1], args[2]),
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
