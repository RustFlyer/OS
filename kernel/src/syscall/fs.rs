use alloc::{ffi::CString, string::ToString};

use simdebug::stop;
use strum::FromRepr;

use config::{
    inode::InodeMode,
    vfs::{AccessFlags, AtFd, AtFlags, MountFlags, OpenFlags, SeekFrom},
};
use driver::BLOCK_DEVICE;
use osfs::{
    FS_MANAGER,
    pipe::{inode::PIPE_BUF_LEN, new_pipe},
};
use systype::{SysError, SyscallResult};
use vfs::{
    file::File,
    kstat::Kstat,
    path::{Path, split_parent_and_name},
};

use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    log::trace!(
        "[sys_openat] dirfd: {dirfd}, pathname: {pathname:#x}, flags: {flags:#x}, mode: {mode:#x}"
    );

    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    // `mode` is not supported yet.
    let _mode = InodeMode::from_bits_truncate(mode);

    let path = {
        let mut addr_space_lock = task.addr_space_mut().lock().await;
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &mut addr_space_lock);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };
    log::trace!("[sys_openat] path: {path}");

    let mut dentry = task.walk_at(AtFd::from(dirfd), path)?;
    // Handle symlinks early here to simplify the logic.
    if !dentry.is_negative() && dentry.inode().unwrap().inotype().is_symlink() {
        if flags.contains(OpenFlags::O_NOFOLLOW) {
            return Err(SysError::ELOOP);
        }
        dentry = Path::resolve_symlink_through(dentry)?;
    }

    if dentry.is_negative() {
        if flags.contains(OpenFlags::O_CREAT) {
            let parent = dentry.parent().unwrap();
            parent.create(dentry.as_ref(), InodeMode::REG)?
        } else {
            return Err(SysError::ENOENT);
        }
    }

    // Now `dentry` must be valid.
    let inode = dentry.inode().unwrap();
    let inode_type = inode.inotype();

    if flags.contains(OpenFlags::O_DIRECTORY) && !inode_type.is_dir() {
        return Err(SysError::ENOTDIR);
    }

    let file = <dyn File>::open(dentry)?;
    file.set_flags(flags);

    task.with_mut_fdtable(|ft| ft.alloc(file, flags))
}

pub async fn sys_write(fd: usize, addr: usize, len: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space_lock = task.addr_space_mut().lock().await;
    let mut data_ptr = UserReadPtr::<u8>::new(addr, &mut addr_space_lock);

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let buf = unsafe { data_ptr.try_into_slice(len) }?;
    file.write(buf).await
}

pub async fn sys_read(fd: usize, buf: usize, count: usize) -> SyscallResult {
    let task = current_task();
    let mut addrspace = task.addr_space_mut().lock().await;
    let mut buf = UserWritePtr::<u8>::new(buf, &mut addrspace);

    let buf_ptr = unsafe { buf.try_into_mut_slice(count) }?;
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    file.read(buf_ptr).await
}

pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> SyscallResult {
    #[derive(FromRepr)]
    #[repr(usize)]
    enum Whence {
        Set = 0,
        Cur = 1,
        End = 2,
    }
    let task = current_task();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let whence = Whence::from_repr(whence).ok_or(SysError::EINVAL)?;

    match whence {
        Whence::Set => file.seek(SeekFrom::Start(offset as u64)),
        Whence::Cur => file.seek(SeekFrom::Current(offset as i64)),
        Whence::End => file.seek(SeekFrom::End(offset as i64)),
    }
}

pub async fn sys_getcwd(buf: usize, len: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
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

pub async fn sys_fstat(fd: usize, stat_buf: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
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

pub async fn sys_mkdirat(dirfd: usize, pathname: usize, mode: u32) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
    let path = UserReadPtr::<u8>::new(pathname, &mut addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    if !dentry.is_negative() {
        return Err(SysError::EEXIST);
    }

    let parent = dentry.parent().ok_or(SysError::ENOENT)?;
    let mode = InodeMode::from_bits_truncate(mode).union(InodeMode::DIR);
    parent.mkdir(dentry.as_ref(), mode)?;
    Ok(0)
}

pub async fn sys_chdir(path: usize) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
    let path = UserReadPtr::<u8>::new(path, &mut addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;
    log::debug!("[sys_chdir] path: {path}");
    let dentry = task.walk_at(AtFd::FdCwd, path)?;
    if !dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }
    task.set_cwd(dentry);
    Ok(0)
}

pub async fn sys_unlinkat(dirfd: usize, pathname: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let flags = AtFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    let path = {
        let mut addr_space_lock = task.addr_space_mut().lock().await;
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &mut addr_space_lock);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };
    log::debug!("[sys_unlinkat] path: {path}");

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    let parent = dentry.parent().expect("can not remove root directory");
    let is_dir = dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir();

    if flags.contains(AtFlags::AT_REMOVEDIR) {
        if !is_dir {
            return Err(SysError::ENOTDIR);
        }
        todo!("remove directory");
    } else if is_dir {
        return Err(SysError::EISDIR);
    }

    parent.unlink(dentry.as_ref()).map(|_| 0)
}

pub async fn sys_getdents64(fd: usize, buf: usize, len: usize) -> SyscallResult {
    log::debug!("[sys_getdents64] fd {fd}, buf {buf:#x}, len {len:#x}");
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let mut ptr = UserWritePtr::<u8>::new(buf, &mut addr_space);
    log::debug!("[sys_getdents64] try to get buf");
    let mut buf = unsafe { ptr.try_into_mut_slice(len) }?;
    log::debug!("[sys_getdents64] try to read dir");
    file.read_dir(&mut buf)
}

/// Implements the `mount` syscall for attaching a filesystem.
///
/// # Arguments (User-space Perspective)
/// - `source`: Pointer to a null-terminated string (C-style) in user memory:
///   - For **device-backed** filesystems (e.g., ext4): Path to block device (e.g., `/dev/sda1`).
///   - For **virtual** filesystems (e.g., procfs): May be empty or a dummy string (e.g., `"none"`).
///   - **Corresponds to**: `dev: Option<Arc<dyn BlockDevice>>` in `mount()`, but passed as a path.
///
/// - `target`: Pointer to a null-terminated string for the mount point path (e.g., `/mnt/usb`).
///   - **Corresponds to**: Combined `parent: Option<Arc<dyn Dentry>>` and `name: &str` in `mount()`,
///     where `target` is the full path (parent + name).
///
/// - `fstype`: Pointer to a null-terminated string for filesystem type (e.g., `"ext4"`, `"proc"`).
///   - **VFS Handling**: Used internally to select the appropriate `FileSystem` implementation.
///   - No direct equivalent in `mount()`, as `mount()` operates on an existing `FileSystem` instance.
///
/// - `flags`: Bitmask of mount options (e.g., `MS_RDONLY`).
///   - **Direct mapping**: Converted to `MountFlags` in `mount()`.
///
/// - `data`: Pointer to additional configuration (often `NULL`).
///   - **Usage**: Filesystem-specific (e.g., NFS server options). May be ignored for simple FS.
///   - No direct equivalent in `mount()` (handled internally by FS drivers).
///
/// # Returns
/// - `Ok(0)` on success.
/// - `Err(SysError)` on failure (e.g., `EINVAL` for invalid flags or paths).
///
/// # Attention
/// - `source` dev is substituted by BLOCK_DEVICE now.
pub async fn sys_mount(
    source: usize,
    target: usize,
    fstype: usize,
    flags: u32,
    data: usize,
) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;

    let mut read_c_str = |ptr| {
        let path = UserReadPtr::<u8>::new(ptr, &mut addr_space).read_c_string(30)?;
        path.into_string().map_err(|_| SysError::EINVAL)
    };

    let source = read_c_str(source)?;
    let target = read_c_str(target)?;
    let fstype = read_c_str(fstype)?;
    let flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    // let data = read_c_str(data)?;

    log::debug!(
        "[sys_mount] source:{source:?}, target:{target:?}, fstype:{fstype:?}, flags:{flags:?}, data:{data:?}",
    );

    let ext4_type = FS_MANAGER.lock().get("ext4").unwrap().clone();
    let fs_type = FS_MANAGER
        .lock()
        .get(&fstype)
        .unwrap_or(&ext4_type.clone())
        .clone();

    if task.pid() > 0 {
        return Ok(0);
    }

    let _fs_root = match fs_type.name().as_str() {
        name @ "ext4" => {
            log::debug!("[sys_mount] ext4 check pass");
            let dev = if name.eq("ext4") {
                log::debug!("[sys_mount] ext4 get block dev");
                Some(BLOCK_DEVICE.get().unwrap().clone())
            } else {
                None
            };
            let (parent, name) = split_parent_and_name(&target);
            log::debug!("[sys_mount] start mount [{}], [{}]", parent, name.unwrap());
            let parent = task.walk_at(AtFd::FdCwd, parent.to_string())?;
            log::debug!("[sys_mount] parent dentry is {}", parent.path());
            fs_type.mount(name.unwrap(), Some(parent), flags, dev)?
        }
        name @ "fat32" => {
            log::debug!("[sys_mount] fat32 check pass");
            let dev = if name.eq("fat32") {
                log::debug!("[sys_mount] fat32 get block dev");
                Some(BLOCK_DEVICE.get().unwrap().clone())
            } else {
                None
            };
            let (parent, name) = split_parent_and_name(&target);
            log::debug!("[sys_mount] start mount [{}], [{}]", parent, name.unwrap());
            let parent = task.walk_at(AtFd::FdCwd, parent.to_string())?;
            log::debug!("[sys_mount] parent dentry is {}", parent.path());
            fs_type.mount(name.unwrap(), Some(parent), flags, dev)?
        }
        _ => return Err(SysError::EINVAL),
    };
    Ok(0)
}

pub async fn sys_umount2(target: usize, flags: u32) -> SyscallResult {
    let task = current_task();
    let mut addr_space = task.addr_space_mut().lock().await;
    let mut ptr = UserReadPtr::<u8>::new(target, &mut addr_space);
    let mount_path = ptr.read_c_string(256);
    let _flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    log::info!("[sys_umount2] umount path:{mount_path:?}");
    Ok(0)
}

/// Checks file permissions relative to a directory
///
/// Verifies whether the calling process can access the file at `pathname` with the
/// specified `mode`.
///
/// # Parameters
/// - `dirfd`: Directory file descriptor (use `AT_FDCWD` for current working directory)
/// - `pathname`: Path string (relative to `dirfd` if not absolute)
/// - `mode`: Permission mask
/// - `flags`: Behavior flags
pub async fn sys_faccessat(dirfd: usize, pathname: usize, mode: i32, flags: i32) -> SyscallResult {
    let task = current_task();
    let _mode = AccessFlags::from_bits(mode).ok_or(SysError::EINVAL)?;
    let flags = AtFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    let path = {
        let mut addr_space_lock = task.addr_space_mut().lock().await;
        let mut user_ptr = UserReadPtr::<u8>::new(pathname, &mut addr_space_lock);
        let cstring = user_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    let mut dentry = task.walk_at(AtFd::from(dirfd), path)?;
    if dentry.is_negative() {
        return Err(SysError::ENOENT);
    }
    if dentry.inode().unwrap().inotype().is_symlink()
        && !flags.contains(AtFlags::AT_SYMLINK_NOFOLLOW)
    {
        dentry = Path::resolve_symlink_through(dentry)?;
        if dentry.is_negative() {
            return Err(SysError::ENOENT);
        }
    }

    // File permissions are not implemented yet, so any access to an existing file is allowed.
    Ok(0)
}

/// set system robust mutex list
///
/// When mutex with attr `PTHREAD_MUTEX_ROBUST` is used, the kernel trace the mutex
/// with this syscall and mark the mutex state as `FUTEX_OWNER_DIED` as the thread
/// dies due to exception.
///
/// # Attention
/// - Not Implemented
pub fn sys_set_robust_list(_robust_list_head: usize, _len: usize) -> SyscallResult {
    Ok(0)
}

pub async fn sys_pipe2(pipefd: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags)
        .unwrap_or_else(|| unimplemented!("unknown flags, should add them"));
    let (pipe_read, pipe_write) = new_pipe(PIPE_BUF_LEN);
    let pipe = task.with_mut_fdtable(|table| {
        let fd_read = table.alloc(pipe_read, flags)?;
        let fd_write = table.alloc(pipe_write, flags)?;
        log::info!("[sys_pipe2] read_fd: {fd_read}, write_fd: {fd_write}, flags: {flags:?}");
        Ok([fd_read as u32, fd_write as u32])
    })?;

    let mut addr_space = task.addr_space_mut().lock().await;
    let mut pipefd = UserWritePtr::<u32>::new(pipefd, &mut addr_space);
    unsafe {
        pipefd.write_array(&pipe)?;
    }
    stop();
    Ok(0)
}
