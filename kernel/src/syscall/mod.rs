mod consts;
mod process;
mod time;

use core::slice;

use ::time::TimeVal;
use consts::SyscallNo::{self, *};
use process::sys_exit;
use time::*;

use crate::{
    print,
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub async fn syscall(syscall_no: usize, args: [usize; 6]) -> usize {
    let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
        log::error!("Syscall number not included: {syscall_no}");
        unimplemented!()
    };

    let result = match syscall_no {
        GETTIMEOFDAY => sys_gettimeofday(args[0], args[1]),
        EXIT => sys_exit(args[0] as i32),
        WRITE => {
            // A temporary implementation for writing to console
            if args[0] == 1 {
                let task = current_task();
                let mut addr_space_lock = task.addr_space_mut().lock();
                let mut data_ptr = UserReadPtr::<u8>::new(args[1], &mut *addr_space_lock);
                match unsafe { data_ptr.read_vector(args[2]) } {
                    Ok(data) => match core::str::from_utf8(&data) {
                        Ok(utf8_str) => {
                            print!("{}", utf8_str);
                            Ok(utf8_str.len())
                        }
                        Err(e) => {
                            log::warn!("Failed to convert string to UTF-8: {:?}", e);
                            log::warn!("String bytes: {:?}", data);
                            unimplemented!()
                        }
                    },
                    Err(e) => {
                        log::warn!("Failed to read string from user space: {:?}", e);
                        unimplemented!()
                    }
                }
            } else {
                log::error!("Unsupported file descriptor: {:}", args[0]);
                unimplemented!()
            }
        }
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
