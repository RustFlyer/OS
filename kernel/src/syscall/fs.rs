use crate::{
    print,
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use alloc::string::{String, ToString};
use config::{inode::InodeMode, vfs::OpenFlags};
use driver::BLOCK_DEVICE;
use log::{debug, error, info};
use mm::{
    address::{PhysAddr, VirtAddr},
    vm::trace_page_table_lookup,
};
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};

use mutex::SleepLock;
use vfs::path::Path;

#[allow(unused)]
static WRITE_LOCK: SleepLock<()> = SleepLock::new(());

pub fn sys_write(fd: usize, addr: usize, len: usize) -> SyscallResult {
    // log::info!("try to write!");
    let task = current_task();
    let mut addr_space_lock = task.addr_space_mut().lock();
    let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut *addr_space_lock);

    if fd == 1 {
        let data = unsafe { data_ptr.read_array(len) }?;
        let utf8_str = core::str::from_utf8(&data).map_err(SysError::from_utf8_err)?;
        print!("{}", utf8_str);
        Ok(utf8_str.len())
    } else {
        debug!("begin to sys write");
        let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
        let buf = unsafe { data_ptr.try_into_slice(len) }?;
        debug!("sys write");
        file.write(buf)
    }
}

pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let mode = InodeMode::from_bits_truncate(mode);

    let pathname = {
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &mut *addr_space_lock);
        match unsafe { data_ptr.read_array(3) } {
            Ok(data) => match core::str::from_utf8(&data) {
                Ok(utf8_str) => utf8_str.to_string(),
                Err(_) => unimplemented!(),
            },
            Err(_) => unimplemented!(),
        }
    };

    debug!("path name = {}", pathname);

    let dentry = {
        let path = Path::new(sys_root_dentry(), sys_root_dentry(), &pathname);
        path.walk().expect("sys_openat: fail to find dentry")
    };

    debug!("flags = {:?}", flags);
    if flags.contains(OpenFlags::O_CREAT) {
        let parent = dentry.parent().expect("can not create with root entry");
        parent.create(&pathname, InodeMode::FILE | mode)?;
    }

    let inode = dentry.inode()?;
    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }

    log::info!("try to open dentry");
    let file = dentry.open()?;
    file.set_flags(flags);

    let root_path = "/".to_string();
    sys_root_dentry().base_open()?.ls(root_path);

    task.with_mut_fdtable(|ft| ft.alloc(file, flags))
}

pub fn sys_read(fd: usize, buf: usize, count: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock();
    let mut buf = UserWritePtr::<u8>::new(buf, &mut addrspace);
    let buf_ptr = unsafe { buf.try_into_mut_slice(count) }?;

    debug!("begin to sys read");
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let ret = file.read(buf_ptr);

    ret
}
