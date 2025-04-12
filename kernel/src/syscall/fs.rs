use alloc::ffi::CString;

use strum::FromRepr;

use config::{
    inode::InodeMode,
    vfs::{AT_REMOVEDIR, AtFd, OpenFlags, SeekFrom},
};
use driver::sbi::getchar;
use mutex::SleepLock;
use systype::{SysError, SyscallResult};
use vfs::{file::File, kstat::Kstat};

use crate::{
    print,
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

#[allow(unused)]
static WRITE_LOCK: SleepLock<()> = SleepLock::new(());

pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let mode = InodeMode::from_bits_truncate(mode);

    let pathname = {
        let mut addr_space_lock = task.addr_space_mut().lock();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &mut *addr_space_lock);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    log::debug!("path name = {}", pathname);
    let dentry = task.resolve_path(AtFd::from(dirfd), pathname)?;

    log::debug!("flags = {:?}", flags);
    if flags.contains(OpenFlags::O_CREAT) {
        let parent = dentry.parent().expect("can not create with root entry");
        parent.create(dentry.as_ref(), InodeMode::REG | mode)?;
    }

    let inode = dentry.inode().ok_or(SysError::ENOENT)?;
    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }

    log::info!("try to open dentry");
    let file = <dyn File>::open(dentry)?;
    file.set_flags(flags);

    log::trace!("file flags: {:?}", file.flags());

    // let root_path = "/".to_string();
    // sys_root_dentry().base_open()?.ls(root_path);

    task.with_mut_fdtable(|ft| ft.alloc(file, flags))
}

pub fn sys_write(fd: usize, addr: usize, len: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space_lock = task.addr_space_mut().lock();
    let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut *addr_space_lock);

    if fd == 1 {
        let data = unsafe { data_ptr.read_array(len) }?;
        let utf8_str = core::str::from_utf8(&data).map_err(SysError::from_utf8_err)?;
        print!("{}", utf8_str);
        Ok(utf8_str.len())
    } else {
        log::debug!("begin to sys write");
        let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
        let buf = unsafe { data_ptr.try_into_slice(len) }?;
        log::debug!("sys write");
        file.write(buf)
    }
}

pub fn sys_read(fd: usize, buf: usize, count: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock();

    let ret = if fd == 0 {
        let mut buf = UserWritePtr::<u8>::new(buf, &mut addrspace);
        let data = getchar();
        unsafe {
            buf.write(data)?;
        };
        Ok(1)
    } else {
        let mut buf = UserWritePtr::<u8>::new(buf, &mut addrspace);
        let buf_ptr = unsafe { buf.try_into_mut_slice(count) }?;
        let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
        file.read(buf_ptr)
    };

    ret
}

pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> SyscallResult {
    #[derive(FromRepr)]
    #[repr(usize)]
    enum Whence {
        SeekSet = 0,
        SeekCur = 1,
        SeekEnd = 2,
    }
    let task = current_task();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let whence = Whence::from_repr(whence).ok_or(SysError::EINVAL)?;

    match whence {
        Whence::SeekSet => file.seek(SeekFrom::Start(offset as u64)),
        Whence::SeekCur => file.seek(SeekFrom::Current(offset as i64)),
        Whence::SeekEnd => file.seek(SeekFrom::End(offset as i64)),
    }
}

pub fn sys_getcwd(buf: usize, len: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let mut buf = { UserWritePtr::<u8>::new(buf, &mut addr_space) };

    let path = task.cwd_mut().path();
    let bsize = core::cmp::min(path.len() + 1, len);

    let cstr = CString::new(path).expect("fail to convert CString");
    let ret = buf.to_usize();
    unsafe {
        buf.try_into_mut_slice(bsize)?
            .copy_from_slice(&cstr.into_bytes_with_nul());
    }
    Ok(ret)
}

pub fn sys_fstat(fd: usize, stat_buf: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let kstat = Kstat::from_vfs_file(file.inode())?;
    unsafe {
        UserWritePtr::<Kstat>::new(stat_buf, &mut addr_space).write(kstat)?;
    }
    Ok(0)
}

pub fn sys_close(fd: usize) -> SyscallResult {
    let task = current_task();
    task.with_mut_fdtable(|table| table.remove(fd))?;
    Ok(0)
}

pub fn sys_dup(fd: usize) -> SyscallResult {
    log::info!("[sys_dup] oldfd: {fd}");
    let task = current_task();
    task.with_mut_fdtable(|table| table.dup(fd))
}

pub fn sys_dup3(oldfd: usize, newfd: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits_truncate(flags);
    assert!(oldfd != newfd);
    task.with_mut_fdtable(|table| table.dup3(oldfd, newfd, flags))
}

pub fn sys_mkdirat(dirfd: usize, pathname: usize, mode: u32) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let path = UserReadPtr::<u8>::new(pathname, &mut addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    let dentry = task.resolve_path(AtFd::from(dirfd), path)?;
    if !dentry.is_negative() {
        return Err(SysError::EEXIST);
    }

    let parent = dentry.parent().ok_or(SysError::ENOENT)?;
    let mode = InodeMode::from_bits_truncate(mode).union(InodeMode::DIR);
    parent.mkdir(dentry.as_ref(), mode)?;
    Ok(0)
}

pub fn sys_chdir(path: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let path = UserReadPtr::<u8>::new(path, &mut addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;
    log::debug!("[sys_chdir] path: {path}");
    let dentry = task.resolve_path(AtFd::FdCwd, path)?;
    if !dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }
    task.set_cwd(dentry);
    Ok(0)
}

pub fn sys_unlinkat(dirfd: usize, pathname: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let path = UserReadPtr::<u8>::new(pathname, &mut addr_space).read_c_string(30)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    log::debug!("[sys_unlinkat] path: {path}");
    let dentry = task.resolve_path(AtFd::from(dirfd), path)?;
    let parent = dentry.parent().expect("can not remove root directory");
    let is_dir = dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir();

    if flags == AT_REMOVEDIR && !is_dir {
        return Err(SysError::ENOTDIR);
    } else if flags != AT_REMOVEDIR && is_dir {
        return Err(SysError::EISDIR);
    }

    parent.unlink(dentry.as_ref()).map(|_| 0)
}

pub fn sys_getdents64(fd: usize, buf: usize, len: usize) -> SyscallResult {
    log::debug!("[sys_getdents64] fd {fd}, buf {buf:#x}, len {len:#x}");
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let mut ptr = UserWritePtr::<u8>::new(buf, &mut addr_space);
    log::debug!("[sys_getdents64] try to get buf");
    let mut buf = unsafe { ptr.try_into_mut_slice(len) }?;
    log::debug!("[sys_getdents64] try to read dir");
    file.read_dir(&mut buf)
}
