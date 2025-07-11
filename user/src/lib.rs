#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;

mod error;
pub mod initproclib;
mod lang_items;
#[allow(unused)]
mod syscall;

extern crate alloc;

use alloc::{ffi::CString, vec::Vec};

use buddy_system_allocator::LockedHeap;
use config::{inode::InodeMode, vfs::OpenFlags};
pub use error::SyscallErr;
// use sig::{Sig, SigAction};
use syscall::*;

pub use initproclib::*;

// const USER_HEAP_SIZE: usize = 16384;
const USER_HEAP_SIZE: usize = 0x32000;

// Note that heap space is allocated in .data segment
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

pub fn getcwd(path: *mut u8, len: usize) -> isize {
    sys_getcwd(path, len)
}

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
    fd: isize,
    offset: usize,
) -> usize {
    sys_mmap(
        addr as usize,
        length,
        prot as usize,
        flags as usize,
        fd as usize,
        offset,
    ) as usize
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

pub fn chdir(path: &str) -> isize {
    let pathname = CString::new(path).unwrap();
    sys_chdir(pathname.as_ptr() as _)
}

pub fn mkdir(path: &str) -> isize {
    let pathname = CString::new(path).unwrap();
    sys_mkdir(-100, pathname.as_ptr() as _, 0)
}

//************ task ***************/
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code);
    panic!("sys_exit should not return");
}
pub fn exit_group(exit_code: i32) -> ! {
    sys_exit_group(exit_code);
    panic!("sys_exit_group should not return");
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

// pub fn prlimit64(pid: usize, resource: i32, new_limit: usize, old_limit: &RLimit) -> isize {
//     sys_prlimit64(
//         pid,
//         resource as usize,
//         new_limit,
//         old_limit as *const RLimit as usize,
//     )
// }

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
    sys_gettimeofday(
        time_val as *mut TimeVal as *mut usize,
        core::ptr::null_mut::<usize>(),
    )
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

pub fn setuid(uid: usize) -> isize {
    sys_setuid(uid)
}
