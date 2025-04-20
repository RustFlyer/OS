use alloc::{ffi::CString, string::ToString};
use core::cmp;

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

/// The `open`() system call opens the file specified by `pathname`.  If the specified file does not ex‐
/// ist, it may optionally (if `O_CREAT` is specified in flags) be created by `open`().
///
/// # Returns
/// The return value of open() is a file descriptor, a small, nonnegative integer that  is  used  in
/// subsequent system calls (`read`(2), `write`(2), `lseek`(2), `fcntl`(2), etc.) to refer to the open file.
/// The file descriptor returned by a successful call will be the  lowest-numbered  file  descriptor
/// not currently open for the process.
/// - default,  the  new  file  descriptor  is  set  to remain open across an `execve`(2) (i.e., the
///   `FD_CLOEXEC` file descriptor flag described in `fcntl`(2)  is  initially  disabled);  the  `O_CLOEXEC`
///   flag, described in `man 2 openat`, can be used to change this default.  
///
/// # Tips
///
/// - The `file offset` is set to the beginning of the file (see `lseek`(2)).
/// - A call to `open()` creates a new open file description, an entry in the system-wide table of  open
///   files.  The open file description records the file offset and the file status flags.
/// - A file descriptor is a reference to an open file description; this reference  is  unaffected  if
///   `pathname`  is subsequently removed or modified to refer to a different file.  For further details
///   on open file descriptions, see `man 2 openat`.
///
/// # Flags
/// The argument `flags` must include one of  the  following  access  modes:  `O_RDONLY`,  `O_WRONLY`,  or
/// `O_RDWR`.  These request opening the file read-only, write-only, or read/write, respectively.
///        
/// In  addition,  zero  or  more  file  creation flags and file status flags can be bitwise-or'd in
/// flags.  
///
/// The file creation flags are `O_CLOEXEC`, `O_CREAT`, `O_DIRECTORY`, `O_EXCL`, `O_NOCTTY`,  `O_NOFOLLOW`
/// , `O_TMPFILE`, and `O_TRUNC`.  
///
/// The file status flags are all of the remaining flags listed in `man 2 openat`.
pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    // `mode` is not supported yet.
    // The mode argument specifies the file mode bits be applied when a new file is created.
    // This argument must be supplied when O_CREAT or O_TMPFILE is specified in flags;
    // Note that this mode applies only to future accesses of the newly created file;
    let _mode = InodeMode::from_bits_truncate(mode);

    let path = {
        let addr_space = task.addr_space();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };
    log::info!(
        "[sys_openat] dirfd: {dirfd}, pathname: {pathname}, flags: {flags:?}, mode: {_mode:?}"
    );

    let mut dentry = task.walk_at(AtFd::from(dirfd), path)?;
    // Handle symlinks early here to simplify the logic.
    if !dentry.is_negative() && dentry.inode().unwrap().inotype().is_symlink() {
        if flags.contains(OpenFlags::O_NOFOLLOW) {
            return Err(SysError::ELOOP);
        }
        dentry = Path::resolve_symlink_through(dentry)?;
    }

    // If pathname does not exist, create it as a regular file.
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

/// `write()`  writes up to `len` bytes from the `addr`(the address of data in memory) to the file
/// referred to by the file descriptor `fd`.
///
/// # Returns
/// On success, the `number` of bytes written is returned.  On error, -1 is returned, and errno is set
/// to indicate the cause of the error.
///
/// # Tips
/// - `write()` allows user to write messages from `addr` to any accessed file, including real and virtual
///   files such as stdout. When user calls `printf` in user space without `close(STDOUT)` + `dup`, the `fd` is STDOUT
///   by default.
/// - This is a `async` syscall, which means that it likely `yield` or `suspend` when called. Therefore, use
///   `lock` carefully and do not pass the `lock` across `await` as possible.
pub async fn sys_write(fd: usize, addr: usize, len: usize) -> SyscallResult {
    log::info!("[sys_write] fd: {fd}, addr: {addr:#x}, len: {len:#x}");

    let task = current_task();
    let addr_space = task.addr_space();
    let mut data_ptr = UserReadPtr::<u8>::new(addr, &addr_space);

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let buf = unsafe { data_ptr.try_into_slice(len) }?;
    file.write(buf).await
}

/// `read()`  attempts  to  read up to `len` bytes from file descriptor `fd` into the buffer starting at
/// `buf`.
///
/// # Returns
/// On  success, the `number` of bytes read is returned (zero indicates end of file), and the file `offset`
/// is advanced by this number.
///
/// # Tips
/// - `read()` allows user to read messages from any accessed file to `addr` , including real and virtual
///   files such as stdin. When user calls `getchar` in user space without `close(STDIN)` + `dup`, the `fd` is STDIN by default.
/// - This is a `async` syscall, which means that it likely `yield` or `suspend` when called. Therefore, use
///   `lock` carefully and do not pass the `lock` across `await` as possible.
pub async fn sys_read(fd: usize, buf: usize, len: usize) -> SyscallResult {
    log::info!("[sys_read] fd: {fd}, buf: {buf:#x}, len: {len:#x}");

    let task = current_task();
    let addr_space = task.addr_space();
    let mut buf = UserWritePtr::<u8>::new(buf, &addr_space);

    let buf_ptr = unsafe { buf.try_into_mut_slice(len) }?;
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    file.read(buf_ptr).await
}

pub fn sys_readlinkat(dirfd: usize, pathname: usize, buf: usize, bufsiz: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let path = UserReadPtr::<u8>::new(pathname, &addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    log::info!("[sys_readlinkat] dirfd: {dirfd}, pathname: {pathname}, bufsiz: {bufsiz:#x}");

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    let inode = dentry.inode().ok_or(SysError::ENOENT)?;
    if !inode.inotype().is_symlink() {
        return Err(SysError::EINVAL);
    }
    let file = <dyn File>::open(dentry).unwrap();
    let link_path = file.readlink()?;
    let mut buf_ptr = UserWritePtr::<u8>::new(buf, &addr_space);
    let len = cmp::min(link_path.len(), bufsiz);
    unsafe {
        buf_ptr
            .try_into_mut_slice(len)?
            .copy_from_slice(&link_path.as_bytes()[..len]);
    }
    Ok(len)
}

/// `lseek()`  repositions  the  file `offset` of the open file description associated with the file
/// descriptor `fd` to the argument `offset` according to the directive `whence` as follows:
/// # Whence
/// - SEEK_SET: The file offset is set to `offset` bytes.
/// - SEEK_CUR: The file offset is set to its `current` location plus `offset` bytes.
/// - SEEK_END: The file offset is set to the `size` of the file plus `offset` bytes.
/// # Tips
/// - `lseek()` allows the file offset to be set **beyond** the `end` of the file (but this does **not change**
///   the `size` of the file).  If data is **later written** at this point, **subsequent reads** of the data in
///   the gap (a "hole") return `null` bytes ('\0') until data is actually written into the gap.
pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> SyscallResult {
    log::info!("[sys_lseek] fd: {fd}, offset: {offset}, whence: {whence}");

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

/// `getcwd()` get current working directory and push it into a `len`-size space `buf`.
///
/// # Returns
/// On success, these functions return a `pointer` to a string containing the `pathname` of the current
/// working directory.
/// On  failure,  these functions return NULL, and `errno` is set to indicate the error.
pub async fn sys_getcwd(buf: usize, len: usize) -> SyscallResult {
    log::info!("[sys_getcwd] len: {len:#x}");

    let task = current_task();
    let addr_space = task.addr_space();
    let mut buf = { UserWritePtr::<u8>::new(buf, &addr_space) };

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

/// `fstat()` get file status.
/// These functions return information about a file, in the buffer pointed to by statbuf.
///
/// # Tips
/// - No permmissions are required on the file itself.
///
/// # Returns
/// return information about a file as a stat struct
/// ```c
/// [rept(C)]
/// struct stat {
///     dev_t     st_dev;         /* ID of device containing file */
///     ino_t     st_ino;         /* Inode number */
///     mode_t    st_mode;        /* File type and mode */
///     nlink_t   st_nlink;       /* Number of hard links */
///     uid_t     st_uid;         /* User ID of owner */
///     gid_t     st_gid;         /* Group ID of owner */
///     dev_t     st_rdev;        /* Device ID (if special file) */
///     off_t     st_size;        /* Total size, in bytes */
///     blksize_t st_blksize;     /* Block size for filesystem I/O */
///     blkcnt_t  st_blocks;      /* Number of 512B blocks allocated */
///     struct timespec st_atim;  /* Time of last access */
///     struct timespec st_mtim;  /* Time of last modification */
///     struct timespec st_ctim;  /* Time of last status change */
/// };
/// ```
pub fn sys_fstat(fd: usize, stat_buf: usize) -> SyscallResult {
    log::info!("[sys_fstat] fd: {fd}");
    let task = current_task();
    let addr_space = task.addr_space();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let kstat = Kstat::from_vfs_file(file.inode())?;
    unsafe {
        UserWritePtr::<Kstat>::new(stat_buf, &addr_space).write(kstat)?;
    }
    Ok(0)
}

pub fn sys_fstatat(dirfd: usize, pathname: usize, stat_buf: usize, _flags: i32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let path = UserReadPtr::<u8>::new(pathname, &addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    log::info!("[sys_fstat_at] dirfd: {dirfd}, path: {path}, flags: {_flags}");
    assert!(
        _flags == 0 || _flags == AtFlags::AT_SYMLINK_NOFOLLOW.bits(),
        "Flags {_flags} is not supported",
    );

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    let inode = dentry.inode().ok_or(SysError::ENOENT)?;
    let kstat = Kstat::from_vfs_file(inode)?;
    unsafe {
        UserWritePtr::<Kstat>::new(stat_buf, &addr_space).write(kstat)?;
    }
    Ok(0)
}

/// `close()` close a file descriptor `fd`.
/// So that the `fd` no longer refers to any file and may be reused.
///
/// # Returns
/// `close()` returns zero on success.  On error, -1 is returned, and errno is set appropriately.
///
/// # Tips
/// - A successful close does not guarantee that the data has been successfully saved to disk,
///   as the kernel uses the buffer cache to defer writes. filesystems do **not flush** buffers when
///   a file is closed.(If wanted, use `fsync` [Not implemented]).
/// - It is probably unwise to close file descriptors while they may be in use by system calls in
///   other threads in the same process, since a file descriptor may be reused.
pub fn sys_close(fd: usize) -> SyscallResult {
    log::info!("[sys_close] fd: {fd}");
    let task = current_task();
    task.with_mut_fdtable(|table| table.remove(fd))?;
    Ok(0)
}

/// `dup()` creates a copy of the file descriptor oldfd, using the lowest-numbered unused
/// file descriptor for the new descriptor.
///
/// # Tips
/// - The OpenFlag is the same between old and new fd.
pub fn sys_dup(fd: usize) -> SyscallResult {
    log::info!("[sys_dup] fd: {fd}");
    let task = current_task();
    task.with_mut_fdtable(|table| table.dup(fd))
}

/// `dup3()` creates a copy of the file descriptor `oldfd`, using the file descriptor number
/// specified in `newfd` as new fd.
///
/// # Tips
/// - If the file descriptor newfd was previously open, it is silently closed before being reused.
///   The steps of closing and reusing the file descriptor newfd are performed atomically.
///   - If oldfd is not a valid file descriptor, then the call fails, and newfd is not closed.
///   - If oldfd is a valid file descriptor, and newfd has the same value as oldfd, then dup3() does
///     fail with the error EINVAL.
/// - The caller can force the close-on-exec flag to be set for the new file descriptor. The flag
///   can close the fd automatically when `sys_execve` is called. It can prevent the fd leaked in
///   the env of multi-threads.
pub fn sys_dup3(oldfd: usize, newfd: usize, flags: i32) -> SyscallResult {
    if oldfd.eq(&newfd) {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let flags = OpenFlags::from_bits_truncate(flags);

    log::info!("[sys_dup3] oldfd: {oldfd}, newfd: {newfd}, flags: {flags:?}");

    task.with_mut_fdtable(|table| table.dup3(oldfd, newfd, flags))
}

/// `mkdirat()` attempts to create a directory named `pathname`.
///
/// # Returns
/// `mkdirat()` return zero on success, or -1 if an error occurred.
///
/// # Dirfd
/// - If `pathname` is relative and `dirfd` is the special value `AT_FDCWD`, then pathname is interpreted
///   relative to the current working directory of the calling process.
/// - If `pathname` is absolute, then `dirfd` is ignored.
///
/// # Todo
/// - Mode Control
pub async fn sys_mkdirat(dirfd: usize, pathname: usize, mode: u32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let path = UserReadPtr::<u8>::new(pathname, &addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    log::info!("[sys_mkdirat] dirfd: {dirfd}, path: {path}, mode: {mode}");

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    if !dentry.is_negative() {
        return Err(SysError::EEXIST);
    }

    let parent = dentry.parent().ok_or(SysError::ENOENT)?;
    let mode = InodeMode::from_bits_truncate(mode).union(InodeMode::DIR);
    parent.mkdir(dentry.as_ref(), mode)?;
    Ok(0)
}

/// `chdir()` changes the current working directory of the calling process to the directory specified
/// in `path`.
///
/// # Returns
/// On success, zero is returned.  On error, -1 is returned, and errno is set appropriately.
///
/// # Tips
/// - A child process created via `fork()` inherits its parent's current working directory. The
///   current working directory is left unchanged by `execve()`.
pub async fn sys_chdir(path: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let path = UserReadPtr::<u8>::new(path, &addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;

    log::info!("[sys_chdir] path: {path}");

    let dentry = task.walk_at(AtFd::FdCwd, path)?;
    if !dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }
    task.set_cwd(dentry);
    Ok(0)
}

/// `unlinkat()` deletes  a name from the filesystem. If that name was the last link to a file and no
/// processes have the file open, the file is deleted and the space it was using is made  available
/// for reuse.
///
/// # Type
/// - If the name was the last link to a file but any **processes** still have the file open, the file
///   will remain in existence until the last file descriptor referring to it is closed.
/// - If the name referred to a `symbolic` link, the link is removed.
/// - If the name referred to a socket, FIFO, or device, the name for it is removed but processes
///   which have the object open may continue to use it.
///
/// # Dirfd
/// - If the `pathname` given in pathname is relative, then it is interpreted relative to the directory
///   referred to by the file descriptor `dirfd`.
/// - If  the  `pathname`  given  in pathname is relative and dirfd is the special value `AT_FDCWD`, then
///   `pathname` is interpreted relative to the current working directory of the calling process.
/// - If the `pathname` given in pathname is absolute, then dirfd is ignored.
pub async fn sys_unlinkat(dirfd: usize, pathname: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let flags = AtFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    let path = {
        let addr_space = task.addr_space();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };
    log::info!("[sys_unlinkat] dirfd: {dirfd}, path: {path}, flags: {flags:?}");

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

/// `getdents64()` get directory entries.
/// The system call getdents() reads several `linux_dirent` structures from the directory referred to
/// by  the  open file descriptor `fd` into the `buf` pointed to by dirp.  The argument `len` specifies
/// the size of the buffer.
/// # linux_dirent
/// ```c
/// #[rept(C)]
/// struct linux_dirent {
///     unsigned long  d_ino;     /* Inode number */
///     unsigned long  d_off;     /* Offset to next linux_dirent */
///     unsigned short d_reclen;  /* Length of this linux_dirent */
///     unsigned char  d_type;    /* File type */
///     char           d_name[];  /* Filename (null-terminated) */
/// }
/// ```
/// # Example
///```md
/// $ ./a.out /testfs/
/// --------------- nread=120 ---------------
/// inode#    file type  d_reclen  d_off   d_name
///        2  directory    16         12  .
///        2  directory    16         24  ..
///       11  directory    24         44  lost+found
///       12  regular      16         56  a
///   228929  directory    16         68  sub
///    16353  directory    16         80  sub2
///   130817  directory    16       4096  sub3
/// ```
pub async fn sys_getdents64(fd: usize, buf: usize, len: usize) -> SyscallResult {
    log::info!("[sys_getdents64] fd {fd}, len {len:#x}");
    let task = current_task();
    let addr_space = task.addr_space();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let mut ptr = UserWritePtr::<u8>::new(buf, &addr_space);
    let buf = unsafe { ptr.try_into_mut_slice(len) }?;
    file.read_dir(buf)
}

/// Implements the `mount` syscall for attaching a filesystem.
///
/// # Arguments
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
    let addr_space = task.addr_space();

    let read_c_str = |ptr| {
        let path = UserReadPtr::<u8>::new(ptr, &addr_space).read_c_string(256)?;
        path.into_string().map_err(|_| SysError::EINVAL)
    };

    let source = read_c_str(source)?;
    let target = read_c_str(target)?;
    let fstype = read_c_str(fstype)?;
    let flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    // let data = read_c_str(data)?;

    log::info!(
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

/// `umount()` remove the attachment of the (topmost) filesystem mounted on target with
/// additional flags controlling the behavior of the operation.
/// # Flags
/// - to write when the basic functions are implemented...
pub async fn sys_umount2(target: usize, flags: u32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let mut ptr = UserReadPtr::<u8>::new(target, &addr_space);
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
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let cstring = user_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    log::info!(
        "[sys_faccessat] dirfd: {dirfd}, pathname: {pathname}, mode: {_mode:?}, flags: {flags:?}"
    );

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

/// `pipe2()` creates a `pipe`, a unidirectional data channel that can be used for interprocess
/// communication with OpenFlags `flags`.
///
/// # Flags
/// - **O_CLOEXEC**: Set the close-on-exec (FD_CLOEXEC) flag on the two new file descriptors.
/// - **O_DIRECT**: Create a pipe that performs I/O in "`packet`" mode.  Each `write(2)` to the pipe is dealt
///   with as a separate packet, and read(2)s from the pipe will read one packet at a time.
/// - **O_NONBLOCK**: Set the O_NONBLOCK file status flag on the open file descriptions referred to by the new
///   file descriptors.  Using this flag saves extra calls to `fcntl(2)` to achieve the same result.
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

    log::info!(
        "[sys_pipe2] pipefd: {:#x}, read_fd: {}, write_fd: {}, flags: {:?}",
        pipefd,
        pipe[0],
        pipe[1],
        flags
    );

    let addr_space = task.addr_space();
    let mut pipefd = UserWritePtr::<u32>::new(pipefd, &addr_space);
    unsafe {
        pipefd.write_array(&pipe)?;
    }
    stop();
    Ok(0)
}
