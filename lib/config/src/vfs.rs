use core::fmt::Display;

use bitflags::bitflags;

bitflags::bitflags! {
    /// This is a bitmask of flags that can be passed to the `open` syscall as parameter
    /// `flags`. It modifies the behavior when accessing and creating the file it opens.
    ///
    /// There are 3 types of flags:
    ///
    /// - File access modes are O_RDONLY, O_WRONLY, and O_RDWR.
    /// - File creation flags are O_CLOEXEC, O_CREAT, O_DIRECTORY, O_EXCL, O_NOCTTY,
    ///   O_NOFOLLOW, O_TMPFILE, and O_TRUNC.
    /// - Other flags are file status flags.
    ///
    /// Constants `ACCESS_MODE`, `CREATION_FLAGS`, and `STATUS_FLAGS` are bitmasks
    /// of the corresponding flags.
    ///
    /// Defined in <bits/fcntl-linux.h>. See `man 2 open` for more information.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: i32 {
        /* File access modes. */

        // Note: `bitflags` crate do not encourage zero bit flag, we should not directly
        // check `O_RDONLY`. Call `readable()` instead.
        const O_RDONLY      = 0;
        const O_WRONLY      = 1;
        const O_RDWR        = 2;

        /* File creation flags. */
        const O_CREAT       = 0o100;
        const O_EXCL        = 0o200;
        const O_NOCTTY      = 0o400;
        const O_TRUNC       = 0o1000;
        const O_DIRECTORY   = 0o200000;
        const O_NOFOLLOW    = 0o400000;
        const O_CLOEXEC     = 0o2000000;
        const O_TMPFILE     = 0o20200000;

        /* File status flags. */
        const O_APPEND      = 0o2000;
        const O_NONBLOCK    = 0o4000;
        const O_DSYNC       = 0o10000;
        const O_ASYNC       = 0o20000;
        const O_DIRECT      = 0o40000;
        const O_LARGEFILE   = 0o100000;
        const O_NOATIME     = 0o1000000;
        const O_SYNC        = 0o4010000;
        const O_RSYNC       = 0o4010000;
        const O_PATH        = 0o10000000;
    }
}

impl OpenFlags {
    /// Bitmask of access modes.
    pub const ACCESS_MODE: Self = Self::O_RDONLY.union(Self::O_WRONLY).union(Self::O_RDWR);

    /// Bitmask of file creation flags.
    pub const CREATION_FLAGS: Self = Self::O_CREAT
        .union(Self::O_EXCL)
        .union(Self::O_NOCTTY)
        .union(Self::O_TRUNC)
        .union(Self::O_DIRECTORY)
        .union(Self::O_NOFOLLOW)
        .union(Self::O_CLOEXEC)
        .union(Self::O_TMPFILE);

    /// Bitmask of file status flags.
    pub const STATUS_FLAGS: Self = Self::O_APPEND
        .union(Self::O_NONBLOCK)
        .union(Self::O_DSYNC)
        .union(Self::O_ASYNC)
        .union(Self::O_DIRECT)
        .union(Self::O_LARGEFILE)
        .union(Self::O_NOATIME)
        .union(Self::O_SYNC)
        .union(Self::O_RSYNC)
        .union(Self::O_PATH);

    /// A file `open`ed with this flags can be read.
    pub fn readable(&self) -> bool {
        // Not being write-only means it is readable.
        !self.contains(Self::O_WRONLY)
    }

    /// A file `open`ed with this flags can be written.
    pub fn writable(&self) -> bool {
        // Being read-write or write-only means it is writable.
        self.contains(Self::O_RDWR) || self.contains(Self::O_WRONLY)
    }

    /// Returns the access mode of the file.
    pub fn access_mode(&self) -> Self {
        self.intersection(Self::ACCESS_MODE)
    }

    /// Returns the status flags of the file.
    pub fn creation_flags(&self) -> Self {
        self.intersection(Self::CREATION_FLAGS)
    }

    /// Returns the status flags of the file.
    pub fn status_flags(&self) -> Self {
        self.intersection(Self::STATUS_FLAGS)
    }
}

bitflags! {
    /// Kernel internal flags for [`File`]s.
    pub struct FileInternalFlags: u32 {
        /// Don't send fanotify events occurred on this file.
        const FMODE_NONOTIFY = 0x0001;
    }
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

bitflags! {
    // See in "bits/poll.h"
    #[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PollEvents: i16 {
        // Event types that can be polled for. These bits may be set in `events' to
        // indicate the interesting event types; they will appear in `revents' to
        // indicate the status of the file descriptor.
        /// There is data to read.
        const IN = 0x001;
        /// There is urgent data to read.
        const PRI = 0x002;
        ///  Writing now will not block.
        const OUT = 0x004;

        // Event types always implicitly polled for. These bits need not be set in
        // `events', but they will appear in `revents' to indicate the status of the
        // file descriptor.
        /// Error condition.
        const ERR = 0x008;
        /// Hang up.
        const HUP = 0x010;
        /// Invalid poll request.
        const INVAL = 0x020;
    }
}

bitflags::bitflags! {
    #[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
    pub struct EpollEvents: u32 {
        // poll/select 兼容事件
        const IN        = 0x001;          // 可读
        const PRI       = 0x002;          // 高优先级可读
        const OUT       = 0x004;          // 可写
        const ERR       = 0x008;          // 错误
        const HUP       = 0x010;          // 对端关闭/挂起
        const INVAL     = 0x020;          // 无效请求

        // epoll 专有事件（需按 Linux 头文件定义补充位）
        const RDNORMAL  = 0x040;          // 普通可读
        const RDBAND    = 0x080;          // 带外可读
        const WRBAND    = 0x100;          // 带外可写

        // epoll 独有高位
        const MSG       = 0x400;          // 消息可读
        const ERR2      = 0x800;          // 冗余错误（兼容）

        // epoll 特色
        const ET        = 0x80000000;     // EPOLLET 边缘触发
        const ONESHOT   = 0x40000000;     // EPOLLONESHOT 只触发一次
        const WAKEUP    = 0x20000000;     // EPOLLWAKEUP 唤醒（很少用）
        const EXCLUSIVE = 0x10000000;     // EPOLLEXCLUSIVE 独占
    }
}

impl From<EpollEvents> for PollEvents {
    fn from(value: EpollEvents) -> Self {
        match value {
            EpollEvents::IN => PollEvents::IN,
            EpollEvents::PRI => PollEvents::PRI,
            EpollEvents::OUT => PollEvents::OUT,
            EpollEvents::ERR => PollEvents::ERR,
            EpollEvents::HUP => PollEvents::HUP,
            EpollEvents::INVAL => PollEvents::INVAL,
            _ => PollEvents::empty(),
        }
    }
}

impl From<PollEvents> for EpollEvents {
    fn from(value: PollEvents) -> Self {
        match value {
            PollEvents::IN => EpollEvents::IN,
            PollEvents::PRI => EpollEvents::PRI,
            PollEvents::OUT => EpollEvents::OUT,
            PollEvents::ERR => EpollEvents::ERR,
            PollEvents::HUP => EpollEvents::HUP,
            PollEvents::INVAL => EpollEvents::INVAL,
            _ => EpollEvents::empty(),
        }
    }
}

bitflags! {
    /// renameat flag
   pub struct RenameFlag: u32 {
       /// Atomically exchange oldpath and newpath.
       /// Both pathnames must exist but may be of different type
       const RENAME_EXCHANGE = 1 << 1;
       /// Don't overwrite newpath of the rename. Return an error if newpath already exists.
       const RENAME_NOREPLACE = 1 << 0;
       /// This operation makes sense only for overlay/union filesystem implementations.
       const RENAME_WHITEOUT = 1 << 2;
   }
}

bitflags! {
    #[derive(Debug,Clone, Copy)]
    pub struct MountFlags:u32 {
        /// This filesystem is mounted read-only.
        const MS_RDONLY = 1;
        /// The set-user-ID and set-group-ID bits are ignored by exec(3) for executable files on this filesystem.
        const MS_NOSUID = 1 << 1;
        /// Disallow access to device special files on this filesystem.
        const MS_NODEV = 1 << 2;
        /// Execution of programs is disallowed on this filesystem.
        const MS_NOEXEC = 1 << 3;
        /// Writes are synched to the filesystem immediately (see the description of O_SYNC in open(2)).
        const MS_SYNCHRONOUS = 1 << 4;
        /// Alter flags of a mounted FS
        const MS_REMOUNT = 1 << 5;
        /// Allow mandatory locks on an FS
        const MS_MANDLOCK = 1 << 6;
        /// Directory modifications are synchronous
        const MS_DIRSYNC = 1 << 7;
        /// Do not follow symlinks
        const MS_NOSYMFOLLOW = 1 << 8;
        /// Do not update access times.
        const MS_NOATIME = 1 << 10;
        /// Do not update directory access times
        const MS_NODEIRATIME = 1 << 11;
        const MS_BIND = 1 << 12;
        const MS_MOVE = 1 << 13;
        const MS_REC = 1 << 14;
        /// War is peace. Verbosity is silence.
        const MS_SILENT = 1 << 15;
        /// VFS does not apply the umask
        const MS_POSIXACL = 1 << 16;
        /// change to unbindable
        const MS_UNBINDABLE = 1 << 17;
        /// change to private
        const MS_PRIVATE = 1 << 18;
        /// change to slave
        const MS_SLAVE = 1 << 19;
        /// change to shared
        const MS_SHARED = 1 << 20;
        /// Update atime relative to mtime/ctime.
        const MS_RELATIME = 1 << 21;
        /// this is a kern_mount call
        const MS_KERNMOUNT = 1 << 22;
        /// Update inode I_version field
        const MS_I_VERSION = 1 << 23;
        /// Always perform atime updates
        const MS_STRICTATIME = 1 << 24;
        /// Update the on-disk [acm]times lazily
        const MS_LAZYTIME = 1 << 25;
        /// These sb flags are internal to the kernel
        const MS_SUBMOUNT = 1 << 26;
        const MS_NOREMOTELOCK = 1 << 27;
        const MS_NOSEC = 1 << 28;
        const MS_BORN = 1 << 29;
        const MS_ACTIVE = 1 << 30;
        const MS_NOUSER = 1 << 31;
    }
}

/// Enumeration of possible methods to seek within an I/O object.
///
/// Copied from `std`.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes.
    Start(u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    End(i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    Current(i64),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(isize)]
pub enum AtFd {
    /// Special value used to indicate the *at functions should use the current
    /// working directory.
    FdCwd = -100,
    /// Normal file descriptor
    Normal(usize),
}

impl Display for AtFd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AtFd::FdCwd => write!(f, "AT_FDCWD"),
            AtFd::Normal(fd) => write!(f, "{}", fd),
        }
    }
}

impl From<isize> for AtFd {
    fn from(value: isize) -> Self {
        match value {
            -100 => AtFd::FdCwd,
            _ => AtFd::Normal(value as usize),
        }
    }
}

impl From<usize> for AtFd {
    fn from(value: usize) -> Self {
        (value as isize).into()
    }
}

bitflags! {
    /// `AT_*` flags for `*at` functions such as `faccessat`, `fstatat`, and `unlinkat`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AtFlags: i32 {
        /// Use the current working directory to determine the target of relative file
        /// paths.
        const AT_FDCWD = -100;
        /// Check access using effective user and group ID.
        const AT_EACCESS = 0x200;
        /// Do not follow symbolic links.
        const AT_SYMLINK_NOFOLLOW = 0x100;
        /// Follow symbolic link.
        const AT_SYMLINK_FOLLOW = 0x400;
        /// Don't automount the terminal ("basename") component of pathname.
        /// Since Linux 3.1 this flag is ignored. Since Linux 4.11 this flag is implied.
        const AT_NO_AUTOMOUNT = 0x800;
        /// Remove directory instead of file.
        const AT_REMOVEDIR = 0x200;
        /// Empty Path
        const AT_EMPTY_PATH  = 0x1000;
        /// Recursively clone the entire subtree
        const AT_RECURSIVE = 0x8000;

        const AT_HANDLE_FID = 0x200;
    }
}

bitflags! {
    /// Test flags for `faccessat` syscall.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AccessFlags: i32 {
        /// Test for read permission.
        const R_OK = 0x4;
        /// Test for write permission.
        const W_OK = 0x2;
        /// Test for execute permission.
        const X_OK = 0x1;
        /// Test for existence of file.
        const F_OK = 0x0;
    }
}

bitflags! {
    pub struct FileSystemFlags:u32{
        /// The file system requires a device.
        const REQUIRES_DEV = 0x1;
        /// The options provided when mounting are in binary form.
        const BINARY_MOUNTDATA = 0x2;
        /// The file system has a subtype. It is extracted from the name and passed in as a parameter.
        const HAS_SUBTYPE = 0x4;
        /// The file system can be mounted by userns root.
        const USERNS_MOUNT = 0x8;
        /// Disables fanotify permission events.
        const DISALLOW_NOTIFY_PERM = 0x10;
        /// The file system has been updated to handle vfs idmappings.
        const ALLOW_IDMAP = 0x20;
        /// FS uses multigrain timestamps
        const MGTIME = 0x40;
        /// The file systen will handle `d_move` during `rename` internally.
        const RENAME_DOES_D_MOVE = 0x8000; //32768
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    // Defined in <stdio.h>.
    pub struct RenameFlags: i32 {
        /// Don't overwrite newpath of the rename. Return an error if newpath already exists.
        const RENAME_NOREPLACE = 1 << 0;
        /// Atomically exchange oldpath and newpath.
        const RENAME_EXCHANGE = 1 << 1;
        /// This operation makes sense only for overlay/union filesystem implementations.
        const RENAME_WHITEOUT = 1 << 2;
    }
}
