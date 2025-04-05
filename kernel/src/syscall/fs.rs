use crate::{print, processor::current_task, vm::user_ptr::UserReadPtr};
use alloc::string::ToString;
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
    if fd == 1 {
        let task = current_task();
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut *addr_space_lock);
        match unsafe { data_ptr.read_array(len) } {
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
        log::error!("Unsupported file descriptor: {:}", fd);
        unimplemented!()
    }
}

pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let mode = InodeMode::from_bits_truncate(mode);

    let pathname = {
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &mut addr_space_lock);
        // match data_ptr.read_c_string(100) {
        match unsafe { data_ptr.read_array(8) } {
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
        parent.create(dentry.as_ref(), InodeMode::REG | mode)?;
    }

    let inode = dentry.inode().unwrap();
    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }

    log::info!("try to open dentry");
    let file = dentry.open()?;
    file.set_flags(flags);

    Ok(0)
}
