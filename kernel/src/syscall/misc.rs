use alloc::vec::Vec;
use arch::time::get_time_duration;
use systype::error::SyscallResult;

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
            release: Self::from_str("6.8.2-42-generic"),
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
    log::info!("uname buf: {buf:#x}");
    let task = current_task();
    let addr_space = task.addr_space();
    let mut ubuf = UserWritePtr::<UtsName>::new(buf, &addr_space);
    if !ubuf.is_null() {
        unsafe {
            log::info!("uname write");
            ubuf.write(UtsName::default())?;
        }
    }
    Ok(0)
}

pub fn sys_syslog(log_type: usize, bufp: usize, len: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut bufp = UserWritePtr::<u8>::new(bufp, &addrspace);
    log::warn!("[sys_log] unimplemeted call log_type: {log_type}");
    match log_type {
        2..=4 => {
            // For type equal to 2, 3, or 4, a successful call to syslog() returns the
            // number of bytes read.
            unsafe {
                bufp.try_into_mut_slice(len)?;
            }
            Ok(0)
        }
        9 => {
            // For type 9, syslog() returns the number of bytes currently available to be
            // read on the kernel log buffer.
            Ok(0)
        }
        10 => {
            // For type 10, syslog() returns the total size of the kernel log buffer.  For
            // other values of type, 0 is returned on success.
            Ok(0)
        }
        _ => {
            // For other values of type, 0 is returned on success.
            Ok(0)
        }
    }
}

pub const SYSINFO_SIZE: usize = size_of::<Sysinfo>();

const _F_SIZE: usize = 20 - 2 * size_of::<u64>() - size_of::<u32>();

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Sysinfo {
    /// Seconds since boot
    pub uptime: i64,
    /// 1, 5, and 15 minute load averages
    pub loads: [u64; 3],
    /// Total usable main memory size
    pub totalram: u64,
    /// Available memory size
    pub freeram: u64,
    /// Amount of shared memory
    pub sharedram: u64,
    /// Memory used by buffers
    pub bufferram: u64,
    /// Total swap space size
    pub totalswap: u64,
    /// swap space still available
    pub freeswap: u64,
    /// Number of current processes
    pub procs: u16,
    /// Explicit padding for m68k
    pub pad: u16,
    /// Total high memory size
    pub totalhigh: u64,
    /// Available high memory size
    pub freehigh: u64,
    /// Memory unit size in bytes
    pub mem_uint: u32,
    /// Padding: libc5 uses this..
    pub _f: [u8; _F_SIZE],
}

impl Sysinfo {
    pub fn collect() -> Self {
        Self {
            uptime: get_time_duration().as_secs() as i64,
            loads: [0; 3],
            totalram: 0,
            freeram: 0,
            sharedram: 0,
            bufferram: 0,
            totalswap: 0,
            freeswap: 0,
            procs: 0,
            pad: 0,
            totalhigh: 0,
            freehigh: 0,
            mem_uint: 0,
            _f: [0; _F_SIZE],
        }
    }
}

pub fn sys_sysinfo(info: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let mut info = UserWritePtr::<Sysinfo>::new(info, &addrspace);
    unsafe {
        info.write(Sysinfo::collect())?;
    }
    Ok(0)
}

/// The `getrandom()` system call fills the buffer pointed to by `buf` with up to `buflen`
/// random bytes. These bytes can be used to seed user-space random number generators
/// or for cryptographic purposes.
///
/// The `flags` argument is a bit mask that can contain zero or more of the following values ORed together:
/// - `GRND_RANDOM`: If this bit is set, then random bytes are drawn from the `random` source
///   (i.e., the same source as the `/dev/random` device) instead of the `urandom` source. The random
///   source is limited based on the entropy that can be obtained from `environmental noise`. If
///   the number of available bytes in the random source is less than requested in `buflen`, the
///   call returns just the available random bytes. If no random bytes are available, the behavior
///   depends on the presence of `GRND_NONBLOCK` in the flags argument.
/// - `GRND_NONBLOCK`:
///   By default, when reading from the random source, `getrandom()` blocks if no random
///   bytes are available, and when reading from the urandom source, it blocks if the entropy pool
///   has not yet been initialized. If the `GRND_NONBLOCK` flag is set, then `getrandom()` does not
///   block in these cases, but instead immediately returns -1 with errno set to `EAGAIN`.
///
/// Attention: For convenience, we choose a simple way to implement it.
pub fn sys_getrandom(buf: usize, buflen: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let mut buf = UserWritePtr::<u8>::new(buf, &addrspace);

    let mut seed = (task.pid() as u64) ^ (buflen as u64) ^ (flags as u64);
    fn simple_rand(mut state: u64) -> u8 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 24) as u8
    }

    let mut random_array: Vec<u8> = Vec::with_capacity(buflen);
    for _ in 0..buflen {
        let byte = simple_rand(seed);
        random_array.push(byte);
        seed = seed.wrapping_add(byte as u64);
    }

    unsafe { buf.write_array(&random_array)? };

    Ok(0)
}
