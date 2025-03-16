mod consts;
mod process;
mod time;

use ::time::TimeVal;
use consts::SyscallNo::{self, *};
use time::*;

pub async fn syscall(syscall_no: usize, args: [usize; 6]) -> usize {
    let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
        log::error!("Syscall number not included: {syscall_no}");
        unimplemented!()
    };

    let result = match syscall_no {
        GETTIMEOFDAY => sys_gettimeofday(args[0] as *const TimeVal, args[1]),
        _ => unimplemented!(),
    };

    match result {
        Ok(ret) => ret,
        Err(e) => {
            log::warn!("[syscall] {syscall_no} return err {e:?}");
            -(e as isize) as usize
        }
    }
}
