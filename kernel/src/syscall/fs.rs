use alloc::{boxed::Box, ffi::CString, string::ToString, sync::Arc, vec::Vec};
use core::{
    cmp, mem,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use strum::FromRepr;

use arch::time::get_time_duration;
use config::{
    device::BLOCK_SIZE,
    inode::InodeMode,
    vfs::{AccessFlags, AtFd, AtFlags, MountFlags, OpenFlags, PollEvents, RenameFlags, SeekFrom},
};
use driver::BLOCK_DEVICE;
use osfs::{
    FS_MANAGER,
    dev::{
        rtc::{RtcTime, ioctl::RtcIoctlCmd},
        tty::{
            TtyIoctlCmd,
            ioctl::{Pid, Termios},
        },
    },
    fd_table::{FdFlags, FdSet},
    pipe::{inode::PIPE_BUF_LEN, new_pipe},
    pselect::{FilePollRet, PSelectFuture},
};
use osfuture::{Select2Futures, SelectOutput};
use systype::{
    error::{SysError, SysResult, SyscallResult},
    time::TimeSpec,
};
use timer::{TimedTaskResult, TimeoutFuture};
use vfs::{
    dentry::Dentry,
    file::File,
    kstat::Kstat,
    path::{Path, split_parent_and_name},
};

use crate::{
    processor::current_task,
    task::{TaskState, sig_members::IntrBySignalFuture, signal::sig_info::SigSet},
    vm::user_ptr::{UserReadPtr, UserReadWritePtr, UserWritePtr},
};

/// The `open`() system call opens the file specified by `pathname`.  If the specified file does not ex‐
/// ist, it may optionally (if `O_CREAT` is specified in flags) be created by `open`().
///
/// # Returns
/// The return value of open() is a file descriptor, a small, nonnegative integer that  is  used  in
/// subsequent system calls (`read`(2), `write`(2), `lseek`(2), `fcntl`(2), etc.) to refer to the open file.
/// The file descriptor returned by a successful call will be the lowest-numbered file descriptor
/// not currently open for the process.
/// - default,  the new file descriptor is set to remain open across an `execve`(2) (i.e., the
///   `FD_CLOEXEC` file descriptor flag described in `fcntl`(2) is initially  disabled); the `O_CLOEXEC`
///   flag, described in `man 2 openat`, can be used to change this default.  
///
/// # Tips
///
/// - The `file offset` is set to the beginning of the file (see `lseek`(2)).
/// - A call to `open()` creates a new open file description, an entry in the system-wide table of  open
///   files.  The open file description records the file offset and the file status flags.
/// - A file descriptor is a reference to an open file description; this reference is unaffected if
///   `pathname` is subsequently removed or modified to refer to a different file. For further details
///   on open file descriptions, see `man 2 openat`.
///
/// # Flags
/// The argument `flags` must include one of the following access modes: `O_RDONLY`, `O_WRONLY`, or
/// `O_RDWR`. These request opening the file read-only, write-only, or read/write, respectively.
///        
/// In addition, zero or more file creation flags and file status flags can be bitwise-or'd in
/// flags.  
///
/// The file creation flags are `O_CLOEXEC`, `O_CREAT`, `O_DIRECTORY`, `O_EXCL`, `O_NOCTTY`,  `O_NOFOLLOW`
/// , `O_TMPFILE`, and `O_TRUNC`.  
///
/// The file status flags are all of the remaining flags listed in `man 2 openat`.
pub async fn sys_openat(dirfd: usize, pathname: usize, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let uid = task.uid();
    let gid = task.get_pgid();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    // `mode` is not supported yet.
    // The mode argument specifies the file mode bits be applied when a new file is created.
    // This argument must be supplied when O_CREAT or O_TMPFILE is specified in flags;
    // Note that this mode applies only to future accesses of the newly created file;
    let mode = InodeMode::from_bits_truncate(mode);

    let path = {
        let addr_space = task.addr_space();
        let mut data_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let cstring = data_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    let name = path.clone();
    log::info!("[sys_openat] dirfd: {dirfd:#x}, path: {path}, flags: {flags:?}, mode: {mode:?}");

    // DEBUG
    if name.contains("cgroup") {
        log::error!("not support cgroup");
        return Err(SysError::EINVAL);
    }

    let mut dentry = task.walk_at(AtFd::from(dirfd), path)?;

    if flags.contains(OpenFlags::O_TMPFILE) {
        let inode = dentry.inode().ok_or(SysError::ENOENT)?;
        let inode_type = inode.inotype();
        if flags.contains(OpenFlags::O_DIRECTORY) && !inode_type.is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let t = get_time_duration().as_micros() as u32;
        let subdentry = dentry.new_neg_child(format!("tmp{}", t).as_str());
        dentry.create(subdentry.as_ref(), InodeMode::REG)?;
        let inode = subdentry.inode().ok_or(SysError::ENOENT)?;
        let file = <dyn File>::open(subdentry)?;
        file.set_flags(OpenFlags::O_RDWR);
        log::debug!("[sys_openat] opened tmpfile {:?}", name);
        inode.set_nlink(0);
        inode.set_time(get_time_duration().into());
        inode.set_mode(mode.union(InodeMode::REG));
        return task.with_mut_fdtable(|ft| ft.alloc(file, flags));
    }

    log::info!("[sys_openat] dentry path: {}", dentry.path());
    // Handle symlinks early here to simplify the logic.
    if !dentry.is_negative() && dentry.inode().unwrap().inotype().is_symlink() {
        log::info!("[sys_openat] non-null dentry is_symlink");
        if flags.contains(OpenFlags::O_NOFOLLOW) {
            return Err(SysError::ELOOP);
        }
        dentry = Path::resolve_symlink_through(dentry)?;
    }

    let _cred = task.perm_mut();
    let cred = _cred.lock();
    let groups = &cred.groups;

    // Create a regular file when `O_CREAT` is specified if the file does not exist.
    if dentry.is_negative() {
        if flags.contains(OpenFlags::O_CREAT) {
            let parent = dentry.parent().unwrap();
            if !parent.inode().unwrap().check_permission(
                uid as u32,
                gid as u32,
                groups,
                AccessFlags::W_OK | AccessFlags::X_OK,
            ) {
                return Err(SysError::EACCES);
            }

            log::debug!("[sys_openat] create a new file");
            parent.create(dentry.as_ref(), InodeMode::REG)?
        } else {
            return Err(SysError::ENOENT);
        }
    }

    // Now `dentry` must be valid.
    let inode = dentry.inode().unwrap();
    let inode_type = inode.inotype();

    if flags.writable() || flags.contains(OpenFlags::O_TRUNC) {
        if !inode.check_permission(uid as u32, gid as u32, groups, AccessFlags::W_OK) {
            return Err(SysError::EACCES);
        }
    }

    if !inode_type.is_dir() && flags.contains(OpenFlags::O_DIRECTORY) {
        return Err(SysError::ENOTDIR);
    }
    if inode_type.is_dir() && flags.writable() {
        return Err(SysError::EISDIR);
    }

    if flags.contains(OpenFlags::O_TRUNC) && flags.writable() {
        inode.set_size(0);
    }

    let file = <dyn File>::open(dentry)?;
    file.set_flags(flags);

    log::debug!("[sys_openat] opened {:?} is_dir: {:?}", name, inode_type);

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
    // log::debug!("[sys_write] fd: {fd}, addr: {addr:#x}, len: {len:#x}");

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
    // log::debug!("[sys_read] fd: {fd}, buf: {buf:#x}, len: {len:#x}");

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

    log::info!("[sys_readlinkat] dirfd: {dirfd}, path: {path}, bufsiz: {bufsiz:#x}");

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    let inode = dentry.inode().ok_or(SysError::ENOENT)?;

    if !inode.inotype().is_symlink() {
        return Err(SysError::EINVAL);
    }

    let file = <dyn File>::open(dentry).unwrap();
    let link_path = file.readlink()?;

    // log::info!("[sys_readlinkat] link_path : [{link_path}]");

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
/// - `SEEK_SET`: The file offset is set to `offset` bytes.
/// - `SEEK_CUR`: The file offset is set to its `current` location plus `offset` bytes.
/// - `SEEK_END`: The file offset is set to the `size` of the file plus `offset` bytes.
/// # Tips
/// - `lseek()` allows the file offset to be set **beyond** the `end` of the file (but this does **not change**
///   the `size` of the file).  If data is **later written** at this point, **subsequent reads** of the data in
///   the gap (a "hole") return `null` bytes ('\0') until data is actually written into the gap.
pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> SyscallResult {
    // log::info!("[sys_lseek] fd: {fd}, offset: {offset}, whence: {whence}");

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
    let kstat = Kstat::from_vfs_inode(file.inode())?;
    unsafe {
        UserWritePtr::<Kstat>::new(stat_buf, &addr_space).write(kstat)?;
    }
    Ok(0)
}

pub fn sys_fstatat(dirfd: usize, pathname: usize, stat_buf: usize, flags: i32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let path = UserReadPtr::<u8>::new(pathname, &addr_space).read_c_string(256)?;
    let path = path.into_string().map_err(|_| SysError::EINVAL)?;
    let flags = AtFlags::from_bits_retain(flags);

    if !(AtFlags::AT_EMPTY_PATH | AtFlags::AT_SYMLINK_NOFOLLOW | AtFlags::AT_NO_AUTOMOUNT)
        .contains(flags)
    {
        log::warn!("[sys_fstatat] flags: illegal flags: {flags:?}");
        return Err(SysError::EINVAL);
    }

    log::info!(
        "[sys_fstat_at] dirfd: {:#x}, path: {}, flags: {:?}",
        dirfd,
        if flags.contains(AtFlags::AT_EMPTY_PATH) {
            "<empty path>"
        } else {
            &path
        },
        flags
    );

    let dentry = {
        if flags.contains(AtFlags::AT_EMPTY_PATH) && path.is_empty() {
            let dirfd = AtFd::from(dirfd);
            match dirfd {
                AtFd::FdCwd => Err(SysError::EINVAL)?,
                AtFd::Normal(fd) => task.with_mut_fdtable(|t| t.get_file(fd))?.dentry(),
            }
        } else {
            let dentry = task.walk_at(AtFd::from(dirfd), path)?;
            if !flags.contains(AtFlags::AT_SYMLINK_NOFOLLOW)
                && !dentry.is_negative()
                && dentry.inode().unwrap().inotype().is_symlink()
            {
                Path::resolve_symlink_through(dentry)?
            } else {
                dentry
            }
        }
    };
    // log::info!("[sys_fstat_at] dentry path: {}", dentry.path());
    let inode = dentry.inode().ok_or(SysError::ENOENT)?;
    let kstat = Kstat::from_vfs_inode(inode)?;
    log::info!("[sys_fstat_at] dentry: {:?}", kstat);
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
    let task = current_task();
    let result = task.with_mut_fdtable(|table| table.dup(fd))?;
    log::info!("[sys_dup] new fd: {:?}", result);
    Ok(result)
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

    let file = task.with_mut_fdtable(|table| table.get_file(oldfd))?;
    let flags = OpenFlags::from_bits_truncate(flags).union(file.flags());

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
    let inode = dentry.inode().unwrap();

    let _cred = task.perm_mut();
    let cred = _cred.lock();
    inode.set_uid(cred.euid);

    let parent_gid = parent.inode().unwrap().get_gid();
    inode.set_gid(parent_gid);

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
    log::info!(
        "[sys_chdir] dentry inotype: {:?}",
        dentry.inode().ok_or(SysError::ENOENT)?.inotype()
    );

    if !dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir() {
        return Err(SysError::ENOTDIR);
    }
    task.set_cwd(dentry);
    Ok(0)
}

/// `fchdir()` changes the current working directory of the calling process to the directory specified
/// in `fd`.
///
/// # Returns
/// On success, zero is returned.  On error, -1 is returned, and errno is set appropriately.
pub async fn sys_fchdir(fd: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    let dentry = file.dentry();
    if !dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir() {
        log::error!("[sys_fchdir] dentry is not a directory");
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

    if path == "/dev/shm/testshm" {
        log::warn!("[sys_unlinkat] stupid return");
        return Ok(0);
    }

    let dentry = task.walk_at(AtFd::from(dirfd), path)?;
    let parent = dentry.parent().ok_or(SysError::EBUSY)?;
    let is_dir = dentry.inode().ok_or(SysError::ENOENT)?.inotype().is_dir();

    if flags.contains(AtFlags::AT_REMOVEDIR) {
        if !is_dir {
            return Err(SysError::ENOTDIR);
        }
        parent.rmdir(dentry.as_ref())?;
    } else {
        if is_dir {
            return Err(SysError::EISDIR);
        }
        parent.unlink(dentry.as_ref())?;
    }
    Ok(0)
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

/// `faccessat()` checks user's permissions for a file
///
/// `faccessat()` checks whether the calling process can access the file pathname.
/// If pathname is a symbolic link, it is dereferenced.
///
/// If the `pathname` given in `pathname` is relative, then it is interpreted relative
/// to the directory referred to by the file descriptor `dirfd`
///
/// Verifies whether the calling process can access the file at `pathname` with the
/// specified `mode`.
///
/// The mode specifies the accessibility check(s) to be performed, and is either the
/// value F_OK, or a mask consisting of the bitwise OR of one or more of R_OK, W_OK,
/// and X_OK. F_OK tests for the existence of the file. R_OK, W_OK, and X_OK test
/// whether the file exists and grants read, write, and execute permissions, respectively.
///
/// Because the Linux kernel's faccessat() system call does not support a flags argument,
/// the glibc faccessat() wrapper function provided in glibc 2.32 and earlier emulates the
/// required functionality using a combination of the faccessat() system call and fstatat(2).
///
/// # Parameters
/// - `dirfd`: Directory file descriptor (use `AT_FDCWD` for current working directory)
/// - `pathname`: Path string (relative to `dirfd` if not absolute)
/// - `mode`: Permission mask
/// - `flags`: Behavior flags
pub async fn sys_faccessat(dirfd: usize, pathname: usize, mode: i32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let access = AccessFlags::from_bits(mode).ok_or(SysError::EINVAL)?;

    let path = {
        let mut user_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let cstring = user_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };
    log::info!("[sys_faccessat] dirfd: {dirfd}, path: {path}, access: {access:?}");

    let mut pdentrylist: Vec<Arc<dyn Dentry>> = Vec::new();

    let mut dentry =
        task.walk_at_with_parents(AtFd::from(dirfd), path.clone(), &mut pdentrylist)?;
    if dentry.is_negative() {
        return Err(SysError::ENOENT);
    }

    if dentry.inode().unwrap().inotype().is_symlink() {
        dentry = Path::resolve_symlink_through(dentry)?;
        if dentry.is_negative() {
            return Err(SysError::ENOENT);
        }
    }

    let inode = dentry.inode().unwrap();
    let _cred = task.perm_mut();
    let cred = _cred.lock();

    let euid = cred.euid;
    let egid = cred.egid;

    let groups: &[u32] = &cred.groups;

    for d in &pdentrylist[..pdentrylist.len() - 1] {
        let inode = d.inode().unwrap();
        if !inode.check_permission(euid, egid, groups, AccessFlags::X_OK) {
            return Err(SysError::EACCES);
        }
    }

    // F_OK: only check existence
    if access.is_empty() {
        return Ok(0);
    }

    if inode.check_permission(euid, egid, groups, access) {
        Ok(0)
    } else {
        Err(SysError::EACCES)
    }
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
    log::warn!("[sys_set_robust_list] unimplemented");
    Ok(0)
}

pub fn sys_get_robust_list(_pid: i32, _robust_list_head: usize, _len_ptr: usize) -> SyscallResult {
    // let Some(task) = TASK_MANAGER.get(pid as usize) else {
    //     return Err(SysError::ESRCH);
    // };
    // if !task.is_leader() {
    //     return Err(SysError::ESRCH);
    // }
    // // UserReadPtr::<RobustListHead>::from(value)
    // len_ptr.write(&task, mem::size_of::<RobustListHead>())?;
    // robust_list_head.write(&task, unsafe {
    //     *task.with_futexes(|futexes| futexes.robust_list.load(Ordering::SeqCst))
    // })?;
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
    Ok(0)
}

/// The `ioctl()` system call manipulates the underlying device parameters of special files.
/// In particular, many operating characteristics of character special files (e.g., terminals)
/// may be controlled with `ioctl()` operations. The argument fd must be an open file descriptor.
pub fn sys_ioctl(fd: usize, request: usize, argp: usize) -> SyscallResult {
    // return Err(SysError::EBUSY);
    log::info!("[sys_ioctl] fd: {fd}, request: {request:#x}, arg: {argp:#x}");
    let task = current_task();
    let addrspace = task.addr_space();
    let mut arg = UserWritePtr::<u8>::new(argp, &addrspace);
    unsafe {
        let len = if let Some(cmd) = TtyIoctlCmd::from_repr(request) {
            match cmd {
                TtyIoctlCmd::TCGETS => core::mem::size_of::<Termios>(),
                TtyIoctlCmd::TIOCGPGRP => core::mem::size_of::<Pid>(),
                TtyIoctlCmd::TCSETS => core::mem::size_of::<Termios>(),
                _ => 0,
            }
        } else if let Some(cmd) = RtcIoctlCmd::from_repr(request as u64) {
            match cmd {
                RtcIoctlCmd::RTC_RD_TIME => core::mem::size_of::<RtcTime>(),
                _ => 0,
            }
        } else {
            0
        };

        // log::debug!("[sys_ioctl] should write with len: {len}");

        let slice = arg.try_into_mut_slice(len)?;

        let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
        file.ioctl(request, slice.as_ptr() as usize)
    }
}

/// `sendfile()` copies data between one file descriptor and another.
/// Because this copying is done within the kernel, `sendfile()` is more efficient than the combination
/// of read(2) and write(2), which would require transferring data to and from user space.
///
/// `in_fd` should be a file descriptor opened for reading and `out_fd` should be a descriptor
/// opened for writing.
///
/// If `offset` is not NULL, then it points to a variable holding the file `offset` from which
/// `sendfile()` will start reading data from in_fd. When `sendfile()` returns, this variable
/// will be set to the `offset` of the byte following the last byte that was read.
///
/// If `offset` is not NULL, then `sendfile()` does not modify the file `offset` of in_fd;
/// otherwise the file `offset` is adjusted to reflect the number of bytes read from in_fd.
///
/// If `offset` is NULL, then data will be read from in_fd starting at the file `offset`,
/// and the file `offset` will be updated by the call.
pub async fn sys_sendfile64(
    out_fd: usize,
    in_fd: usize,
    offset: usize,
    mut count: usize,
) -> SyscallResult {
    let task = current_task();
    let in_file = task.with_mut_fdtable(|table| table.get_file(in_fd))?;
    let out_file = task.with_mut_fdtable(|table| table.get_file(out_fd))?;

    if offset != 0 {
        in_file.seek(SeekFrom::Start(offset as u64))?;
    }

    let mut write_bytes = 0;
    while count > 0 {
        let mlen = count.min(4096);
        let mut buf = vec![0; mlen];
        let rlen = in_file.read(&mut buf).await?;
        write_bytes += out_file.write(&buf[..rlen]).await?;
        count -= rlen;

        // log::info!("read bytes {}", rlen);
        if rlen == 0 {
            break;
        }
    }

    Ok(write_bytes)
}

// Defined in <bits/fcntl-linux.h>
#[derive(FromRepr, Debug, Eq, PartialEq, Clone, Copy, Default)]
#[allow(non_camel_case_types)]
#[repr(isize)]
pub enum FcntlOp {
    F_DUPFD = 0,
    F_DUPFD_CLOEXEC = 1030,
    F_GETFD = 1,
    F_SETFD = 2,
    F_GETFL = 3,
    F_SETFL = 4,
    #[default]
    F_UNIMPL,
}

/// `fcntl()` performs one of the operations described below on the open file descriptor `fd`.
/// The operation is determined by `op`.
///
/// `fcntl()` can take an optional third argument. Whether or not this argument is required
/// is determined by `op`. The required argument type is indicated in parentheses after
/// each `op` name (in most cases, the required type is int, and we identify the argument
/// using the name arg), or void is specified if the argument is not required.
///
/// # Op
/// - `F_DUPFD`: Duplicate the file descriptor `fd` using the lowest-numbered available
///   file descriptor greater than or equal to arg. This is different from dup2,
///   which uses exactly the file descriptor specified.
/// - `F_DUPFD_CLOEXEC`: As `F_DUPFD`, but additionally set the close-on-exec flag for
///   the duplicate file descriptor. Specifying this flag permits a program to avoid
///   an additional `fcntl()` `F_SETFD` operation to set the FD_CLOEXEC flag.
pub fn sys_fcntl(fd: usize, op: isize, arg: usize) -> SyscallResult {
    use FcntlOp::*;
    let task = current_task();
    let op = FcntlOp::from_repr(op).unwrap_or_default();
    log::debug!("[sys_fcntl] fd: {fd}, op: {op:?}, arg: {arg:#x}");
    match op {
        F_DUPFD_CLOEXEC => {
            task.with_mut_fdtable(|table| table.dup_with_bound(fd, arg, OpenFlags::O_CLOEXEC))
        }
        F_GETFL => {
            let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
            Ok(file.flags().bits() as _)
        }
        F_GETFD => task.with_mut_fdtable(|table| {
            let fd_info = table.get(fd)?;
            log::debug!("[sys_fcntl] {:?}", fd_info.flags());
            Ok(fd_info.flags().bits() as usize)
        }),
        F_SETFD => {
            let arg = OpenFlags::from_bits_retain(arg as i32);
            let fd_flags = FdFlags::from(arg);
            task.with_mut_fdtable(|table| {
                let fd_info = table.get_mut(fd)?;
                fd_info.set_flags(fd_flags);
                Ok(0)
            })
        }
        _ => {
            log::error!("[sys_fcntl] not implemented {op:?}");
            Ok(0)
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IoVec {
    pub base: usize,
    pub len: usize,
}

/// `sys_readv()` read data from file into multiple buffers
pub async fn sys_readv(fd: usize, iov: usize, iovcnt: usize) -> SyscallResult {
    // log::info!("[sys_readv] fd: {fd}, iov: {iov:#x}, iovcnt: {iovcnt}");

    let task = current_task();
    let addrspace = task.addr_space();

    let iovs = {
        let mut iovs_ptr = UserReadPtr::<IoVec>::new(iov, &addrspace);
        unsafe { iovs_ptr.read_array(iovcnt)? }
    };

    // log::info!("[sys_readv] iov: {:?}", iovs);

    let mut read_bytes = 0;
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    for iov in iovs {
        if iov.len == 0 {
            continue;
        }
        let mut ptr = UserWritePtr::<u8>::new(iov.base, &addrspace);
        let slice = unsafe { ptr.try_into_mut_slice(iov.len)? };
        read_bytes += file.read(slice).await?;
    }

    Ok(read_bytes)
}

/// `sys_writev()` write data into file from multiple buffers
pub async fn sys_writev(fd: usize, iov: usize, iovcnt: usize) -> SyscallResult {
    // log::info!("[sys_writev] fd: {fd}, iov: {iov:#x}, iovcnt: {iovcnt}");

    let task = current_task();
    let addrspace = task.addr_space();

    let iovs = {
        let mut iovs_ptr = UserReadPtr::<IoVec>::new(iov, &addrspace);
        unsafe { iovs_ptr.read_array(iovcnt)? }
    };

    // log::info!("[sys_writev] iov: {:?}", iovs);

    let mut write_bytes = 0;
    let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
    for iov in iovs {
        if iov.len == 0 {
            continue;
        }
        let mut ptr = UserReadPtr::<u8>::new(iov.base, &addrspace);
        let slice = unsafe { ptr.try_into_slice(iov.len)? };
        write_bytes += file.write(slice).await?;
    }

    // log::info!("[sys_writev] write bytes: {:?}", write_bytes);
    Ok(write_bytes)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

pub type Async<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub struct PollFuture<'a> {
    futures: Vec<Async<'a, PollEvents>>,
    ready_cnt: usize,
}

impl Future for PollFuture<'_> {
    type Output = Vec<(usize, PollEvents)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut ret_vec = Vec::new();
        for (i, future) in this.futures.iter_mut().enumerate() {
            let result = unsafe { Pin::new_unchecked(future).poll(cx) };
            if let Poll::Ready(result) = result {
                this.ready_cnt += 1;
                ret_vec.push((i, result))
            }
        }
        if this.ready_cnt > 0 {
            log::debug!("[PollFuture] ready: {}", this.ready_cnt);
            Poll::Ready(ret_vec)
        } else {
            Poll::Pending
        }
    }
}

pub fn dyn_future<'a, T: Future + Send + 'a>(async_blk: T) -> Async<'a, T::Output> {
    Box::pin(async_blk)
}

/// `sys_ppoll` waits for one of a set of file descriptors to become ready to perform I/O.
/// The set of file descriptors to be monitored is specified in the `fds` argument, which
/// is an array of structures of the following form:
/// ```c
/// struct pollfd {
///     int   fd;         /* file descriptor */
///     short events;     /* requested events */
///     short revents;    /* returned events */
/// };
/// ```
/// The caller should specify the number of items in the `fds` array in `nfds`.
///
/// The field `fd` contains a file descriptor for an open file. If this field is negative,
/// then the corresponding `events` field is ignored and the `revents` field returns zero.
///
/// The field `events` is an input parameter, a bit mask specifying the `events` the application is
/// interested in for the file descriptor fd. This field may be specified as zero, in which case
/// the only `events` that can be returned in `revents` are POLLHUP, POLLERR, and POLLNVAL
///
/// The field `revents` is an output parameter, filled by the kernel with the `events` that actually
/// occurred. The bits returned in `revents` can include any of those specified in `events`, or one
/// of the values POLLERR, POLLHUP, or POLLNVAL.
///
/// If none of the events requested (and no error) has occurred for any of the file descriptors,
/// then `poll()` blocks until one of the events occurs.
///
/// The timeout argument specifies the number of milliseconds that poll() should block waiting
/// for a file descriptor to become ready. The call will block until either:
/// - a file descriptor becomes ready
/// - the call is interrupted by a signal handler
/// - the timeout expires.
///
/// Being "ready" means that the requested operation will not block;
/// thus, poll()ing regular files, block devices, and other files with no reasonable polling
/// semantic always returns instantly as ready to read and write.
///
/// ppoll() allows an application to safely wait until either a file descriptor
/// becomes ready or until a signal is caught.
///
/// If the sigmask argument is specified as NULL, then no signal mask manipulation is
/// performed (and thus ppoll() differs from poll() only in the precision of the timeout argument).
///
/// The tmo_p argument specifies an upper limit on the amount of time that ppoll() will block.
///
/// If tmo_p is specified as NULL, then ppoll() can block indefinitely.
pub async fn sys_ppoll(fds: usize, nfds: usize, tmo_p: usize, sigmask: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let mut poll_fds = unsafe { UserReadPtr::<PollFd>::new(fds, &addrspace).read_array(nfds)? };

    let time_out = if tmo_p == 0 {
        None
    } else {
        let timespec = unsafe { UserReadPtr::<TimeSpec>::new(tmo_p, &addrspace).read()? };
        Some(Duration::from_micros(timespec.into_ms() as u64))
    };
    log::debug!(
        "[sys_ppoll] poll_fds: {:?}, nfds: {nfds}, timeout: {:?}",
        poll_fds,
        time_out
    );

    let mut futures = Vec::<Async<PollEvents>>::with_capacity(nfds);
    for poll_fd in poll_fds.iter() {
        let fd = poll_fd.fd as usize;
        let events = PollEvents::from_bits(poll_fd.events).unwrap();
        let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
        let future = dyn_future(async move { file.poll(events).await });
        futures.push(future);
    }

    let poll_future = PollFuture {
        futures,
        ready_cnt: 0,
    };

    task.set_state(TaskState::Interruptible);
    task.set_wake_up_signal(!*task.sig_mask_mut());
    let ret_vec = if let Some(timeout) = time_out {
        match TimeoutFuture::new(timeout, poll_future).await {
            TimedTaskResult::Completed(ret_vec) => ret_vec,
            TimedTaskResult::Timeout => {
                log::debug!("[sys_ppoll]: timeout");
                return Ok(0);
            }
        }
    } else {
        let intr_future = IntrBySignalFuture {
            task: task.clone(),
            mask: *task.sig_mask_mut(),
        };
        match Select2Futures::new(poll_future, intr_future).await {
            SelectOutput::Output1(ret_vec) => ret_vec,
            SelectOutput::Output2(_) => return Err(SysError::EINTR),
        }
    };

    task.set_state(TaskState::Running);
    let ret = ret_vec.len();
    for (i, result) in ret_vec {
        poll_fds[i].revents |= result.bits();
    }

    unsafe { UserWritePtr::<PollFd>::new(fds, &addrspace).write_array(&poll_fds)? };

    Ok(ret)
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
pub struct StatFs {
    /// 是个 magic number，每个知名的 fs 都各有定义，但显然我们没有
    pub f_type: i64,
    /// 最优传输块大小
    pub f_bsize: i64,
    /// 总的块数
    pub f_blocks: u64,
    /// 还剩多少块未分配
    pub f_bfree: u64,
    /// 对用户来说，还有多少块可用
    pub f_bavail: u64,
    /// 总的 inode 数
    pub f_files: u64,
    /// 空闲的 inode 数
    pub f_ffree: u64,
    /// 文件系统编号，但实际上对于不同的OS差异很大，所以不会特地去用
    pub f_fsid: [i32; 2],
    /// 文件名长度限制，这个OS默认FAT已经使用了加长命名
    pub f_namelen: isize,
    /// 片大小
    pub f_frsize: isize,
    /// 一些选项，但其实也没用到
    pub f_flags: isize,
    /// 空余 padding
    pub f_spare: [isize; 4],
}

pub fn sys_statfs(path: usize, buf: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let cpath = UserReadPtr::<u8>::new(path, &addrspace).read_c_string(256)?;
    let path = cpath.into_string().expect("cstring fail to convert");

    log::info!("[sys_statfs] path: {path}");

    let stfs = StatFs {
        f_type: 0x20259527,
        f_bsize: BLOCK_SIZE as i64,
        f_blocks: 1 << 27,
        f_bfree: 1 << 26,
        f_bavail: 1 << 20,
        f_files: 1 << 10,
        f_ffree: 1 << 9,
        f_fsid: [0; 2],
        f_namelen: 1 << 8,
        f_frsize: 1 << 9,
        f_flags: 1 << 1,
        f_spare: [0; 4],
    };

    unsafe {
        UserWritePtr::<StatFs>::new(buf, &addrspace).write(stfs)?;
    }

    Ok(0)
}

/// The utime() system call changes the access and modification times of the
/// inode specified by filename to the actime and modtime fields of times
/// respectively. The status change time (ctime) will be set to the current
/// time, even if the other time stamps don't actually change.
///
/// If the tv_nsec field of one of the timespec structures has the special
/// value UTIME_NOW, then the corresponding file timestamp is set to the
/// current time. If the tv_nsec field of one of the timespec structures has
/// the special value UTIME_OMIT, then the corresponding file timestamp
/// is left unchanged. In both of these cases, the value of the
/// corresponding tv_sec field is ignored.
///
/// If times is NULL, then the access and modification times of the file are
/// set to the current time.
pub fn sys_utimensat(dirfd: usize, pathname: usize, times: usize, flags: i32) -> SyscallResult {
    const UTIME_NOW: usize = 0x3fffffff;
    const UTIME_OMIT: usize = 0x3ffffffe;

    let task = current_task();
    let addrspace = task.addr_space();
    let mut pathname = UserReadPtr::<u8>::new(pathname, &addrspace);
    let mut times = UserReadPtr::<TimeSpec>::new(times, &addrspace);
    let dirfd = AtFd::from(dirfd);

    let inode = if !pathname.is_null() {
        let path = pathname
            .read_c_string(256)?
            .into_string()
            .expect("cstring convert failed");
        log::info!("[sys_utimensat] dirfd: {dirfd}, path: {path}");
        let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
        let dentry = task.walk_at(dirfd, path)?;
        dentry.inode().ok_or(SysError::ENOENT)?
    } else {
        // NOTE: if `pathname` is NULL, acts as futimens
        log::info!("[sys_utimensat] fd: {dirfd}");
        match dirfd {
            AtFd::FdCwd => return Err(SysError::EINVAL),
            AtFd::Normal(fd) => {
                let file = task.with_mut_fdtable(|table| table.get_file(fd))?;
                file.inode()
            }
        }
    };

    let mut inner = inode.get_meta().inner.lock();
    let current_time = TimeSpec::from(get_time_duration());
    if times.is_null() {
        log::info!("[sys_utimensat] times is null, update with current time");
        inner.atime = current_time;
        inner.mtime = current_time;
        inner.ctime = current_time;
    } else {
        let times = unsafe { times.read_array(2)? };
        log::info!("[sys_utimensat] times {:?}", times);
        match times[0].tv_nsec {
            UTIME_NOW => inner.atime = current_time,
            UTIME_OMIT => {}
            _ => inner.atime = times[0],
        };
        match times[1].tv_nsec {
            UTIME_NOW => inner.mtime = current_time,
            UTIME_OMIT => {}
            _ => inner.mtime = times[1],
        };
        inner.ctime = current_time;
    }

    Ok(0)
}

/// `renameat2` renames old path name as new path name.
///
pub fn sys_renameat2(
    olddirfd: usize,
    oldpath: usize,
    newdirfd: usize,
    newpath: usize,
    flags: i32,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let olddirfd = AtFd::from(olddirfd);
    let newdirfd = AtFd::from(newdirfd);

    let mut oldpath = UserReadPtr::<u8>::new(oldpath, &addrspace);
    let mut newpath = UserReadPtr::<u8>::new(newpath, &addrspace);

    let flags = RenameFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    let coldpath = oldpath.read_c_string(256)?;
    let cnewpath = newpath.read_c_string(256)?;

    let oldpath = coldpath.into_string().expect("convert to string failed");
    let newpath = cnewpath.into_string().expect("convert to string failed");

    log::info!(
        "[sys_renameat2] olddirfd:{olddirfd:?}, oldpath:{oldpath}, newdirfd:{newdirfd:?}, newpath:{newpath}, flags:{flags:?}"
    );

    //OpenFlag::NO_FOLLOW
    let old_dentry = task.walk_at(olddirfd, oldpath)?;
    let new_dentry = task.walk_at(newdirfd, newpath)?;

    let parent_dentry = old_dentry.parent().expect("can not rename root dentry");
    // old_dentry.rename_to(&new_dentry, flags).map(|_| 0)
    if old_dentry.is_negative() {
        parent_dentry.lookup(old_dentry.name())?;
    }
    parent_dentry.rename(
        old_dentry.as_ref(),
        parent_dentry.as_ref(),
        new_dentry.as_ref(),
    )?;

    // log::error!("[sys_renameat2] implement rename");
    Ok(0)
}

/// `linkat()` makes a new name for a file. It creates a new link (also known as a hard link)
/// to an existing file. If `newpath` exists, it will not be overwritten.
///
/// If the pathname given in `oldpath` is relative, then it is interpreted relative to
/// the directory referred to by the file descriptor `olddirfd` (rather than relative to
/// the current working directory of the calling process, as is done by link() for a
/// relative pathname).
///
/// If `oldpath` is relative and `olddirfd` is the special value AT_FDCWD, then `oldpath` is
/// interpreted relative to the current working directory of the calling process (like
/// link()).
///
/// If `oldpath` is absolute, then `olddirfd` is ignored.
pub fn sys_linkat(
    olddirfd: usize,
    oldpath: usize,
    newdirfd: usize,
    newpath: usize,
    flags: i32,
) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();
    let _flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    let coldpath = UserReadPtr::<u8>::new(oldpath, &addrspace).read_c_string(256)?;
    let cnewpath = UserReadPtr::<u8>::new(newpath, &addrspace).read_c_string(256)?;

    let oldpath = coldpath.into_string().map_err(|_| SysError::EINVAL)?;
    let newpath = cnewpath.into_string().map_err(|_| SysError::EINVAL)?;

    let olddirfd = AtFd::from(olddirfd);
    let newdirfd = AtFd::from(newdirfd);

    let old_dentry = task.walk_at(olddirfd, oldpath)?;
    let new_dentry = task.walk_at(newdirfd, newpath)?;

    new_dentry.link(old_dentry.as_ref(), new_dentry.as_ref())?;
    Ok(0)
}

/// `symlink()` creates a symbolic link named `linkpath` which contains the string `target`.
///
/// Symbolic links are interpreted at run time as if the contents of the link had been
/// substituted into the path being followed to find a file or directory.
///
/// A symbolic link (also known as a soft link) may point to an existing file or to
/// a nonexistent one; the latter case is known as a dangling link.
///
/// The permissions of a symbolic link are irrelevant; the ownership is ignored when
/// following the link (except when the protected_symlinks feature is enabled, as
/// explained in proc(5)), but is checked when removal or renaming of the link is
/// requested and the link is in a directory with the sticky bit (S_ISVTX) set.
///
/// If `linkpath` exists, it will not be overwritten.
pub fn sys_symlinkat(target: usize, newdirfd: usize, linkpath: usize) -> SyscallResult {
    let task = current_task();
    let addrspace = task.addr_space();

    let ctarget = UserReadPtr::<u8>::new(target, &addrspace).read_c_string(256)?;
    let clinkpath = UserReadPtr::<u8>::new(linkpath, &addrspace).read_c_string(256)?;

    let target = ctarget.into_string().map_err(|_| SysError::EINVAL)?;
    let linkpath = clinkpath.into_string().map_err(|_| SysError::EINVAL)?;

    log::info!("[sys_symlinkat] target: {target}, newdirfd: {newdirfd:?}, linkpath: {linkpath}");

    let newdirfd = AtFd::from(newdirfd);

    let dentry = task.walk_at(newdirfd, linkpath)?;
    if !dentry.is_negative() {
        return Err(SysError::EEXIST);
    }
    dentry.parent().unwrap().symlink(dentry.as_ref(), &target)?;
    Ok(0)
}

/// `sync()` causes all pending modifications to filesystem metadata and
/// cached file data to be written to the underlying filesystems.
pub fn sys_sync() -> SyscallResult {
    log::warn!("[sys_sync] not implemented.");
    Ok(0)
}

/// `fsync()` causes all pending modifications to filesystem metadata and
/// cached file data to be written to the underlying filesystems.
pub fn sys_fsync(_fd: usize) -> SyscallResult {
    log::warn!("[sys_fsync] not implemented.");
    Ok(0)
}

/// umask() sets the calling process's file mode creation mask (umask) to
/// mask & 0777 (i.e., only the file permission bits of mask are used),
/// and returns the previous value of the mask.
///
/// The umask is used by open(2), mkdir(2), and other system calls that
/// create files to modify the permissions placed on newly created files
/// or directories. Specifically, permissions in the umask are turned off
/// from the mode argument to open(2) and mkdir(2).
pub fn sys_umask(_mask: i32) -> SyscallResult {
    Ok(0x777)
}

/// The `ftruncate()` functions cause the regular file named by path or
/// referenced by fd to be truncated to a size of precisely length bytes.
///
/// If the file previously was larger than this size, the extra data is lost. If the file
/// previously was shorter, it is extended, and the extended part reads as null bytes ('\0').
/// The file offset is not changed.
///
/// If the size changed, then the st_ctime and st_mtime fields (respectively, time of last
/// status change and time of last modification; see inode(7)) for the file are updated, and
/// the set-user-ID and set-group-ID mode bits may be cleared.
///
/// With ftruncate(), the file must be open for writing; with truncate(), the file
/// must be writable.
pub fn sys_ftruncate(fd: usize, length: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_mut_fdtable(|t| t.get_file(fd))?;

    let inode = file.dentry().inode().ok_or(SysError::ENOENT)?;

    inode.set_size(length);

    Ok(0)
}

pub async fn sys_pselect6(
    nfds: i32,
    readfds1: usize,
    writefds1: usize,
    exceptfds1: usize,
    timeout: usize,
    sigmask: usize,
) -> SyscallResult {
    if nfds.is_negative() {
        return Err(SysError::EINVAL);
    }

    let task = current_task();
    let addrspace = task.addr_space();

    // macro_rules! make_convert {
    //     ($ty:ty) => {
    //         |up: usize| -> SysResult<Option<$ty>> {
    //             let mut ptr = UserReadWritePtr::<$ty>::new(up, &addrspace);
    //             if ptr.is_null() {
    //                 Ok(None)
    //             } else {
    //                 let r = unsafe { ptr.read() }?;
    //                 Ok(Some(r))
    //             }
    //         }
    //     };
    // }

    // let pconvert = make_convert!(FdSet);
    // let tconvert = make_convert!(TimeSpec);
    // let sconvert = make_convert!(SigSet);

    log::info!("[sys_pselect6] timeout: {:#x}", timeout);

    let pconvert = |up: usize| -> SysResult<Option<FdSet>> {
        let mut ptr = UserReadWritePtr::<FdSet>::new(up, &addrspace);
        if ptr.is_null() {
            Ok(None)
        } else {
            let r = unsafe { ptr.read() }?;
            Ok(Some(r))
        }
    };

    let tconvert = |up: usize| -> SysResult<Option<TimeSpec>> {
        let mut ptr = UserReadWritePtr::<TimeSpec>::new(up, &addrspace);
        if ptr.is_null() {
            Ok(None)
        } else {
            let r = unsafe { ptr.read() }?;
            Ok(Some(r))
        }
    };

    let sconvert = |up: usize| -> SysResult<Option<SigSet>> {
        let mut ptr = UserReadWritePtr::<SigSet>::new(up, &addrspace);
        if ptr.is_null() {
            Ok(None)
        } else {
            let r = unsafe { ptr.read() }?;
            Ok(Some(r))
        }
    };

    let writeback = |fdset: Option<FdSet>, addr: usize| -> SysResult<usize> {
        if fdset.is_none() {
            return Ok(0);
        }
        let fdset = fdset.unwrap();
        let mut ptr = UserReadWritePtr::<FdSet>::new(addr, &addrspace);
        log::debug!("fdset: {:?}", fdset);
        unsafe {
            ptr.write(fdset)?;
        }
        Ok(0)
    };

    let nfds = nfds as usize;
    let mut readfds = pconvert(readfds1)?;
    let mut writefds = pconvert(writefds1)?;
    let mut exceptfds = pconvert(exceptfds1)?;
    let timeout = tconvert(timeout)?;
    let sigmask = sconvert(sigmask)?;

    log::debug!("[sys_pselect6] thread: {} call", task.tid());
    log::info!(
        "[sys_pselect6] readfds: {readfds:?}, writefds: {writefds:?}, exceptfds: {exceptfds:?}"
    );
    log::info!("[sys_pselect6] timeout: {:?}", timeout);

    // if let Some(t) = timeout {
    //     if t.is_zero() {
    //         return Ok(0);
    //     }
    // }

    let mut polls = Vec::<FilePollRet>::with_capacity(nfds);

    for fd in 0..nfds {
        let mut events = PollEvents::empty();

        readfds
            .as_ref()
            .map(|fds| fds.is_set(fd).then(|| events.insert(PollEvents::IN)));

        writefds
            .as_ref()
            .map(|fds| fds.is_set(fd).then(|| events.insert(PollEvents::OUT)));

        if !events.is_empty() {
            let file = task.with_mut_fdtable(|f| f.get_file(fd))?;
            log::debug!("[sys_pselect6] fd:{fd}, file path:{}", file.dentry().path());
            polls.push((fd, events, file));
        }
    }

    let old_mask = sigmask.map(|mask| mem::replace(task.sig_mask_mut(), mask));

    task.set_state(TaskState::Interruptible);
    task.set_wake_up_signal(!task.get_sig_mask());

    let intr_future = IntrBySignalFuture::new(task.clone(), task.get_sig_mask());
    let pselect_future = PSelectFuture::new(polls);

    let mut sweep_and_cont = || {
        if let Some(fds) = readfds.as_mut() {
            fds.clear()
        }
        if let Some(fds) = writefds.as_mut() {
            fds.clear()
        }
        if let Some(fds) = exceptfds.as_mut() {
            fds.clear()
        }
        task.set_state(TaskState::Running);

        if let Some(mask) = old_mask {
            *task.sig_mask_mut() = mask;
        }
    };

    let ret_vec = if let Some(timeout) = timeout {
        match Select2Futures::new(
            TimeoutFuture::new(timeout.into(), pselect_future),
            intr_future,
        )
        .await
        {
            SelectOutput::Output1(time_output) => match time_output {
                TimedTaskResult::Completed(ret_vec) => ret_vec,
                TimedTaskResult::Timeout => {
                    log::debug!("[sys_pselect6]: timeout");
                    sweep_and_cont();
                    writeback(readfds, readfds1)?;
                    writeback(writefds, writefds1)?;
                    writeback(exceptfds, exceptfds1)?;
                    osfuture::yield_now().await;
                    return Ok(0);
                }
            },
            SelectOutput::Output2(_) => return Err(SysError::EINTR),
        }
    } else {
        match Select2Futures::new(pselect_future, intr_future).await {
            SelectOutput::Output1(ret_vec) => ret_vec,
            SelectOutput::Output2(_) => return Err(SysError::EINTR),
        }
    };
    sweep_and_cont();

    let mut ret = 0;
    for (fd, events) in ret_vec {
        if events.contains(PollEvents::IN) || events.contains(PollEvents::HUP) {
            log::info!("read ready fd {fd}");
            if let Some(fds) = readfds.as_mut() {
                fds.set(fd)
            }
            ret += 1;
        }
        if events.contains(PollEvents::OUT) {
            log::info!("write ready fd {fd}");
            if let Some(fds) = writefds.as_mut() {
                fds.set(fd)
            }
            ret += 1;
        }
    }

    log::debug!("[sys_pselect6] thread: {} exit", task.tid());
    writeback(readfds, readfds1)?;
    writeback(writefds, writefds1)?;
    writeback(exceptfds, exceptfds1)?;

    Ok(ret)
}

/// `pread()` reads up to `count` bytes from file descriptor `fd` at offset `offset`
/// (from the start of the file) into the buffer starting at `buf`.
///
/// The file offset is not changed.
pub async fn sys_pread64(fd: usize, buf: usize, count: usize, offset: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let mut buf = UserWritePtr::<u8>::new(buf, &addr_space);

    let buf_ptr = unsafe { buf.try_into_mut_slice(count) }?;

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    file.seek(SeekFrom::Start(offset as u64))?;

    file.read(buf_ptr).await
}

/// `pwrite()` writes up to `count` bytes from the buffer starting at `buf` to the file descriptor
/// `fd` at offset `offset`.
///
/// The file offset is not changed.
pub async fn sys_pwrite64(fd: usize, buf: usize, count: usize, offset: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let mut data_ptr = UserReadPtr::<u8>::new(buf, &addr_space);

    let buf = unsafe { data_ptr.try_into_slice(count) }?;

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    file.seek(SeekFrom::Start(offset as u64))?;

    file.write(buf).await
}

/// This function returns information about a file, storing it in the buffer pointed to by statxbuf. The returned buffer is a structure of the following type:
/// ```c
/// struct statx {
///     __u32 stx_mask;        /* Mask of bits indicating
///                               filled fields */
///     __u32 stx_blksize;     /* Block size for filesystem I/O */
///     __u64 stx_attributes;  /* Extra file attribute indicators */
///     __u32 stx_nlink;       /* Number of hard links */
///     __u32 stx_uid;         /* User ID of owner */
///     __u32 stx_gid;         /* Group ID of owner */
///     __u16 stx_mode;        /* File type and mode */
///     __u64 stx_ino;         /* Inode number */
///     __u64 stx_size;        /* Total size in bytes */
///     __u64 stx_blocks;      /* Number of 512B blocks allocated */
///     __u64 stx_attributes_mask;
///                            /* Mask to show what's supported
///                               in stx_attributes */
///     /* The following fields are file timestamps */
///     struct statx_timestamp stx_atime;  /* Last access */
///     struct statx_timestamp stx_btime;  /* Creation */
///     struct statx_timestamp stx_ctime;  /* Last status change */
///     struct statx_timestamp stx_mtime;  /* Last modification */
///     /* If this file represents a device, then the next two
///        fields contain the ID of the device */
///     __u32 stx_rdev_major;  /* Major ID */
///     __u32 stx_rdev_minor;  /* Minor ID */
///     /* The next two fields contain the ID of the device
///        containing the filesystem where the file resides */
///     __u32 stx_dev_major;   /* Major ID */
///     __u32 stx_dev_minor;   /* Minor ID */
///     __u64 stx_mnt_id;      /* Mount ID */
///     /* Direct I/O alignment restrictions */
///     __u32 stx_dio_mem_align;
///     __u32 stx_dio_offset_align;
/// };
/// ```
/// The file timestamps are structures of the following type:
/// ```c
/// struct statx_timestamp {
///     __s64 tv_sec;    /* Seconds since the Epoch (UNIX time) */
///     __u32 tv_nsec;   /* Nanoseconds since tv_sec */
/// };
/// ```
pub fn sys_statx(
    dirfd: usize,
    pathname: usize,
    flags: usize,
    mask: usize,
    statxbuf: usize,
) -> SyscallResult {
    #[repr(C)]
    #[derive(Debug, Default)]
    pub struct StatxTimestamp {
        pub tv_sec: i64,     // __s64
        pub tv_nsec: u32,    // __u32
        pub __reserved: i32, // int
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct Statx {
        stx_mask: u32,
        stx_blksize: u32,
        stx_attributes: u64,
        stx_nlink: u32,
        stx_uid: u32,
        stx_gid: u32,
        stx_mode: u16,
        __spare0: u16,
        stx_ino: u64,
        stx_size: u64,
        stx_blocks: u64,
        stx_attributes_mask: u64,
        pub stx_atime: StatxTimestamp,
        pub stx_btime: StatxTimestamp,
        pub stx_ctime: StatxTimestamp,
        pub stx_mtime: StatxTimestamp,
        pub stx_rdev_major: u32,
        pub stx_rdev_minor: u32,
        pub stx_dev_major: u32,
        pub stx_dev_minor: u32,
        pub stx_mnt_id: u64,
        pub stx_dio_mem_align: u32,
        pub stx_dio_offset_align: u32,
        pub __spare2: [u64; 12], // 预留
    }

    bitflags::bitflags! {
        #[derive(Default)]
        pub struct StatxMask: u32 {
            const TYPE        = 0x00000001; // STATX_TYPE
            const MODE        = 0x00000002; // STATX_MODE
            const NLINK       = 0x00000004; // STATX_NLINK
            const UID         = 0x00000008; // STATX_UID
            const GID         = 0x00000010; // STATX_GID
            const ATIME       = 0x00000020; // STATX_ATIME
            const MTIME       = 0x00000040; // STATX_MTIME
            const CTIME       = 0x00000080; // STATX_CTIME
            const INO         = 0x00000100; // STATX_INO
            const SIZE        = 0x00000200; // STATX_SIZE
            const BLOCKS      = 0x00000400; // STATX_BLOCKS
            const BASIC_STATS = 0x000007ff; // common mask
            const BTIME       = 0x00000800; // STATX_BTIME (Linux 4.11+)
            const ALL         = 0x00000fff; // all
        }
    }

    bitflags::bitflags! {
        #[derive(Default)]
        pub struct StatxFlags: u32 {
            /// AT_SYMLINK_NOFOLLOW: Do not follow symbolic links.
            const SYMLINK_NOFOLLOW  = 0x100;  // AT_SYMLINK_NOFOLLOW
            /// AT_NO_AUTOMOUNT: Suppress terminal automount traversal
            const NO_AUTOMOUNT      = 0x800;  // AT_NO_AUTOMOUNT
            /// AT_EMPTY_PATH: Allow empty relative pathname
            const EMPTY_PATH        = 0x1000; // AT_EMPTY_PATH
            // statx-only flags:
            /// STATX_FORCE_SYNC: Force synchronised I/O, as per description
            const STATX_FORCE_SYNC  = 0x2000; // STATX_FORCE_SYNC
            /// STATX_DONT_SYNC: Don't sync before reading attributes
            const STATX_DONT_SYNC   = 0x4000; // STATX_DONT_SYNC
        }
    }

    let task = current_task();
    let addrspace = task.addr_space();
    let pathname = UserReadPtr::<u8>::new(pathname, &addrspace).read_c_string(256)?;
    let path = pathname.into_string().map_err(|_| SysError::EINVAL)?;
    let dirfd = AtFd::from(dirfd);
    let flags = StatxFlags::from_bits_truncate(flags as u32);

    let from_timespec = |ts: TimeSpec| StatxTimestamp {
        tv_sec: ts.tv_sec as i64,
        tv_nsec: ts.tv_nsec as u32,
        __reserved: 0,
    };

    log::debug!(
        "[sys_statx] path: {}, dirfd: {:?}, flags: {:#x}, mask: {:#x}",
        path,
        dirfd,
        flags,
        mask
    );

    let dentry = {
        if flags.contains(StatxFlags::EMPTY_PATH) {
            let dirfd: AtFd = dirfd;
            match dirfd {
                AtFd::FdCwd => Err(SysError::EINVAL)?,
                AtFd::Normal(fd) => task.with_mut_fdtable(|t| t.get_file(fd))?.dentry(),
            }
        } else {
            let dentry = task.walk_at(dirfd, path)?;
            if !flags.contains(StatxFlags::SYMLINK_NOFOLLOW)
                && !dentry.is_negative()
                && dentry.inode().unwrap().inotype().is_symlink()
            {
                Path::resolve_symlink_through(dentry)?
            } else {
                dentry
            }
        }
    };

    // log::debug!("[sys_statx] dentry opened {:?}", dentry.path());

    let nmask = StatxMask::TYPE
        | StatxMask::MODE
        | StatxMask::NLINK
        | StatxMask::INO
        | StatxMask::SIZE
        | StatxMask::BLOCKS;

    let stat = dentry.inode().ok_or(SysError::ENOENT)?.get_attr()?;
    let statx = Statx {
        stx_mask: nmask.bits(),
        stx_blksize: stat.st_blksize,
        stx_attributes: 0,
        stx_nlink: stat.st_nlink,
        stx_uid: stat.st_uid,
        stx_gid: stat.st_gid,
        __spare0: 0,
        stx_mode: stat.st_mode as u16,
        stx_ino: stat.st_ino,
        stx_size: stat.st_size,
        stx_blocks: stat.st_blocks,
        stx_attributes_mask: 0,
        stx_atime: from_timespec(stat.st_atime),
        stx_btime: StatxTimestamp::default(),
        stx_ctime: from_timespec(stat.st_ctime),
        stx_mtime: from_timespec(stat.st_mtime),
        stx_rdev_major: 0,
        stx_rdev_minor: 0,
        stx_dev_major: 0,
        stx_dev_minor: 0,
        stx_mnt_id: 0,
        stx_dio_mem_align: 0,
        stx_dio_offset_align: 0,
        __spare2: [0; 12],
    };

    // log::warn!("{:?}", statx);

    unsafe {
        UserWritePtr::<Statx>::new(statxbuf, &addrspace).write(statx)?;
    }

    Ok(0)
}

pub fn sys_fchmodat(dirfd: isize, pathname_ptr: usize, mode: u32, flags: u32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let pathname = UserReadPtr::<u8>::new(pathname_ptr, &addr_space).read_c_string(4096)?;
    let pathname = pathname.into_string().map_err(|_| SysError::EINVAL)?;

    let flags = AtFlags::from_bits_retain(flags as i32);

    // if !task.can_chmod(&file) {
    //     return Err(SysError::EPERM);
    // }

    let dentry = {
        if flags.contains(AtFlags::AT_EMPTY_PATH) && pathname.is_empty() {
            let dirfd = AtFd::from(dirfd);
            match dirfd {
                AtFd::FdCwd => Err(SysError::EINVAL)?,
                AtFd::Normal(fd) => task.with_mut_fdtable(|t| t.get_file(fd))?.dentry(),
            }
        } else {
            let dentry = task.walk_at(AtFd::from(dirfd), pathname)?;
            if !flags.contains(AtFlags::AT_SYMLINK_NOFOLLOW)
                && !dentry.is_negative()
                && dentry.inode().unwrap().inotype().is_symlink()
            {
                Path::resolve_symlink_through(dentry)?
            } else {
                dentry
            }
        }
    };

    let inode = dentry.inode().ok_or(SysError::ENOENT)?;

    let rmode = inode.get_meta().inner.lock().mode;

    dentry.inode().ok_or(SysError::ENOENT)?.set_mode(
        rmode
            .intersection(!InodeMode::S_PERM)
            .union(InodeMode::from_bits_retain(mode)),
    );

    Ok(0)
}

pub fn sys_fchownat(
    dirfd: isize,
    pathname_ptr: usize,
    owner: u32,
    group: u32,
    flags: u32,
) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let pathname = UserReadPtr::<u8>::new(pathname_ptr, &addr_space).read_c_string(4096)?;
    let pathname = pathname.into_string().map_err(|_| SysError::EINVAL)?;

    let flags = AtFlags::from_bits_retain(flags as i32);

    let dentry = {
        if flags.contains(AtFlags::AT_EMPTY_PATH) && pathname.is_empty() {
            let dirfd = AtFd::from(dirfd);
            match dirfd {
                AtFd::FdCwd => Err(SysError::EINVAL)?,
                AtFd::Normal(fd) => task.with_mut_fdtable(|t| t.get_file(fd))?.dentry(),
            }
        } else {
            let dentry = task.walk_at(AtFd::from(dirfd), pathname)?;
            if !flags.contains(AtFlags::AT_SYMLINK_NOFOLLOW)
                && !dentry.is_negative()
                && dentry.inode().unwrap().inotype().is_symlink()
            {
                Path::resolve_symlink_through(dentry)?
            } else {
                dentry
            }
        }
    };

    let inode = dentry.inode().ok_or(SysError::ENOENT)?;

    let (old_uid, old_gid) = {
        let inner = inode.get_meta().inner.lock();
        (inner.uid, inner.gid)
    };

    log::debug!(
        "chown: owner={:#x}, group={:#x}, old_uid={}, old_gid={}",
        owner,
        group,
        old_uid,
        old_gid
    );

    let mut changed = false;
    if owner != u32::MAX && owner != old_uid {
        inode.set_uid(owner);
        changed = true;
    }
    if group != u32::MAX && group != old_gid {
        inode.set_gid(group);
        changed = true;
    }

    if changed {
        let mut mode = inode.get_meta().inner.lock().mode;

        mode.remove(InodeMode::SET_UID);
        mode.remove(InodeMode::SET_GID);

        inode.set_mode(mode);
    }
    Ok(0)
}

pub fn sys_close_range(first: usize, last: usize, flags: usize) -> SyscallResult {
    const CLOSE_RANGE_UNSHARE: usize = 1;
    const CLOSE_RANGE_CLOEXEC: usize = 2;

    if first > last {
        return Err(SysError::EINVAL);
    }

    if flags & !(CLOSE_RANGE_CLOEXEC | CLOSE_RANGE_UNSHARE) != 0 {
        return Err(SysError::EINVAL);
    }

    let task = current_task();

    if flags == CLOSE_RANGE_UNSHARE {
        let mut fdtable = task.fdtable_mut().lock().clone();
        fdtable.remove_with_range(first, last, flags)?;
        *task.fdtable_mut().lock() = fdtable;
    } else {
        task.with_mut_fdtable(|table| table.remove_with_range(first, last, flags))?;
    }
    Ok(0)
}

pub async fn sys_copy_file_range(
    fd_in: usize,
    off_in_ptr: usize,
    fd_out: usize,
    off_out_ptr: usize,
    len: usize,
    flags: usize,
) -> SyscallResult {
    if flags != 0 {
        return Err(SysError::EINVAL);
    }

    let task = current_task();
    let addr_space = task.addr_space();

    let file_in = task.with_mut_fdtable(|table| table.get_file(fd_in))?;
    let file_out = task.with_mut_fdtable(|table| table.get_file(fd_out))?;

    if !file_in.flags().readable() || !file_out.flags().writable() {
        return Err(SysError::EBADF);
    }

    let mut off_in = if off_in_ptr == 0 {
        None
    } else {
        unsafe { Some(UserReadPtr::<u64>::new(off_in_ptr, &addr_space).read()?) }
    };

    let mut off_out = if off_out_ptr == 0 {
        None
    } else {
        unsafe { Some(UserReadPtr::<u64>::new(off_out_ptr, &addr_space).read()?) }
    };

    let bytes_copied = file_out
        .copy_file_range(file_in.as_ref(), off_in.as_mut(), off_out.as_mut(), len)
        .await?;

    if let Some(off) = off_in {
        unsafe { UserWritePtr::<u64>::new(off_in_ptr, &addr_space).write(off)? };
    }

    if let Some(off) = off_out {
        unsafe { UserWritePtr::<u64>::new(off_out_ptr, &addr_space).write(off)? };
    }

    Ok(bytes_copied)
}

pub fn sys_fsetxattr(
    fd: usize,
    name_ptr: usize,
    value_ptr: usize,
    size: usize,
    flags: i32,
) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();

    let name = {
        let mut name_ptr = UserReadPtr::<u8>::new(name_ptr, &addr_space);
        let cstring = name_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    let value = if size > 0 {
        let mut value_ptr = UserReadPtr::<u8>::new(value_ptr, &addr_space);
        unsafe { value_ptr.read_array(size)? }
    } else {
        Vec::new()
    };

    let xattr_flags = flags;

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let inode = file.inode();

    inode.set_xattr(&name, &value, xattr_flags)?;

    Ok(0)
}

pub fn sys_fgetxattr(fd: usize, name_ptr: usize, value_ptr: usize, size: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();

    let name = {
        let mut name_ptr = UserReadPtr::<u8>::new(name_ptr, &addr_space);
        let cstring = name_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let inode = file.inode();

    let value = inode.get_xattr(&name)?;

    if value_ptr == 0 {
        return Ok(value.len());
    }

    let copy_len = core::cmp::min(size, value.len());
    let mut user_value_ptr = UserWritePtr::<u8>::new(value_ptr, &addr_space);
    unsafe {
        user_value_ptr
            .try_into_mut_slice(copy_len)?
            .copy_from_slice(&value[..copy_len]);
    }

    Ok(copy_len)
}

pub fn sys_fremovexattr(fd: usize, name_ptr: usize) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();

    let name = {
        let mut name_ptr = UserReadPtr::<u8>::new(name_ptr, &addr_space);
        let cstring = name_ptr.read_c_string(256)?;
        cstring.into_string().map_err(|_| SysError::EINVAL)?
    };

    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let inode = file.inode();

    inode.remove_xattr(&name)?;

    Ok(0)
}
