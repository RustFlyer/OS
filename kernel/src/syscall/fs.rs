use core::sync::atomic::AtomicBool;

use crate::{
    print,
    processor::current_hart,
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use systype::SyscallResult;

use mutex::{SleepLock, SpinNoIrqLock};

static WriteLock: SleepLock<()> = SleepLock::new(());
static MySpinLock: AtomicBool = AtomicBool::new(false);

pub fn sys_write(fd: usize, addr: usize, len: usize) -> SyscallResult {
    // WriteLock.lock();
    loop {
        if MySpinLock
            .compare_exchange(
                false,
                true,
                core::sync::atomic::Ordering::Relaxed,
                core::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            break;
        }
    }

    if fd == 1 {
        let task = current_task();
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut *addr_space_lock);
        match unsafe { data_ptr.read_array(len) } {
            Ok(data) => match core::str::from_utf8(&data) {
                Ok(utf8_str) => {
                    print!("{}", utf8_str);
                    MySpinLock.fetch_and(false, core::sync::atomic::Ordering::Relaxed);
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
        log::error!("Unsupported file descriptor: {:}", fd);
        unimplemented!()
    }
}
