use log::info;
use systype::SyscallResult;

use crate::{processor::current_task, vm::user_ptr::UserWritePtr};

// See in "sys/utsname.h"
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UtsName {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

impl UtsName {
    pub fn default() -> Self {
        Self {
            sysname: Self::from_str("Linux"),
            nodename: Self::from_str("Linux"),
            release: Self::from_str("5.19.0-42-generic"),
            version: Self::from_str(
                "#43~22.04.1-Ubuntu SMP PREEMPT_DYNAMIC Fri Apr 21 16:51:08 UTC 2",
            ),
            machine: Self::from_str("RISC-V SiFive Freedom U740 SoC"),
            domainname: Self::from_str("localhost"),
        }
    }

    fn from_str(info: &str) -> [u8; 65] {
        let mut data: [u8; 65] = [0; 65];
        data[..info.len()].copy_from_slice(info.as_bytes());
        data
    }
}

pub async fn sys_uname(buf: usize) -> SyscallResult {
    info!("uname buf: {buf:#x}");
    let task = current_task();
    let addr_space= task.addr_space();
    let mut ubuf = UserWritePtr::<UtsName>::new(buf, &addr_space);
    if !ubuf.is_null() {
        unsafe {
            info!("uname write");
            ubuf.write(UtsName::default())?;
        }
    }
    Ok(0)
}
