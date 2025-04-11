use alloc::{ffi::CString, string::ToString};

use driver::sbi::getchar;
use mm::address::VirtAddr;
use strum::FromRepr;

use config::{
    inode::InodeMode,
    vfs::{AtFd, OpenFlags, SeekFrom},
};
use mutex::SleepLock;
use osfs::sys_root_dentry;
use systype::{SysError, SyscallResult};
use vfs::{file::File, kstat::Kstat};

use crate::{
    print,
    processor::current_task,
    vm::{
        mem_perm::MemPerm,
        mmap::{MmapFlags, MmapProt},
        user_ptr::{UserReadPtr, UserWritePtr},
    },
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
        match data_ptr.read_c_string(30) {
            Ok(data) => match core::str::from_utf8(&data) {
                Ok(utf8_str) => utf8_str.to_string(),
                Err(_) => unimplemented!(),
            },
            Err(_) => unimplemented!(),
        }
    };

    log::debug!("path name = {}", pathname);
    let dentry = task.resolve_path(AtFd::from(dirfd), pathname)?;

    log::debug!("flags = {:?}", flags);
    if flags.contains(OpenFlags::O_CREAT) {
        let parent = dentry.parent().expect("can not create with root entry");
        parent.create(dentry.as_ref(), InodeMode::REG | mode)?;
    }

    let inode = dentry.inode().ok_or(SysError::EDOM)?;
    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }

    log::info!("try to open dentry");
    let file = <dyn File>::open(dentry)?;
    file.set_flags(flags);

    let root_path = "/".to_string();
    sys_root_dentry().base_open()?.ls(root_path);

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

pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: usize,
    offset: usize,
) -> SyscallResult {
    let task = current_task();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let flags = MmapFlags::from_bits_truncate(flags);
    let prot = MmapProt::from_bits_truncate(prot);
    let perm = MemPerm::from_mmapprot(prot);
    let va = VirtAddr::new(addr);

    log::info!("[sys_mmap] addr:{addr:?} prot:{prot:?}, flags:{flags:?}, perm:{perm:?}");

    if addr == 0 && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    task.addr_space_mut()
        .lock()
        .map_file(file, flags, prot, va, length, offset)
}
