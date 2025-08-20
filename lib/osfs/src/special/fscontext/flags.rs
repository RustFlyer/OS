use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsopenFlags: u32 {
        /// Close-on-exec flag for the returned file descriptor
        const FSOPEN_CLOEXEC = 0x80000;

        /// O_RDONLY - Open for reading only
        const O_RDONLY = 0x0000;

        /// O_WRONLY - Open for writing only
        const O_WRONLY = 0x0001;

        /// O_RDWR - Open for reading and writing
        const O_RDWR = 0x0002;

        /// O_CREAT - Create the file if it does not exist
        const O_CREAT = 0x0040;

        /// O_EXCL - Ensure that this call creates a new file
        const O_EXCL = 0x0080;

        /// O_NOFOLLOW - Do not follow symbolic links
        const O_NOFOLLOW = 0x0100;

        /// O_TRUNC - Truncate the file to zero length if it already exists
        const O_TRUNC = 0x0200;

        /// O_APPEND - Append to the file, if possible
        const O_APPEND = 0x0400;

        /// O_NONBLOCK - Open in non-blocking mode
        const O_NONBLOCK = 0x0800;

        /// O_SYNC - Open with synchronous I/O
        const O_SYNC = 0x1000;

        /// O_DSYNC - Open with data synchronization
        const O_DSYNC = 0x2000;

        /// O_RSYNC - Open with read synchronization
        const O_RSYNC = 0x4000;

        /// O_LARGEFILE - Allow large file operations (for 32-bit systems)
        const O_LARGEFILE = 0x8000;

        /// O_TMPFILE - Create a temporary file (without a pathname)
        const O_TMPFILE = 0x200000;

        /// O_NOATIME - Do not update file's access time
        const O_NOATIME = 0x100000;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy,PartialEq, Eq)]
    pub struct FsConfigCmd: u32 {
        /// Set a string parameter
        const FSCONFIG_SET_STRING = 1;
        /// Set a string parameter with val specified as a string
        const FSCONFIG_SET_BINARY = 0;
        /// Set a parameter using a path
        const FSCONFIG_SET_PATH = 2;
        /// Set a parameter using a path, but with AT_EMPTY_PATH
        const FSCONFIG_SET_PATH_EMPTY = 3;
        /// Set a parameter using a file descriptor
        const FSCONFIG_SET_FD = 5;
        /// Clear a parameter flag
        const FSCONFIG_SET_FLAG = 7;
        /// Create the superblock
        const FSCONFIG_CMD_CREATE = 6;
        /// Reconfigure the superblock
        const FSCONFIG_CMD_RECONFIGURE = 8;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsmountFlags: u32 {
        /// Close-on-exec flag for the returned file descriptor
        const FSMOUNT_CLOEXEC = 0x01;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsContextPhase: u8 {
        /// Awaiting parameters
        const FS_CONTEXT_CREATE_PARAMS = 0;
        /// Creating filesystem
        const FS_CONTEXT_CREATING = 1;
        /// Awaiting mount
        const FS_CONTEXT_AWAITING_MOUNT = 2;
        /// Awaiting reconfiguration parameters
        const FS_CONTEXT_RECONF_PARAMS = 3;
        /// Reconfiguring filesystem
        const FS_CONTEXT_RECONFIGURING = 4;
        /// Failed operation
        const FS_CONTEXT_FAILED = 5;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsContextPurpose: u8 {
        /// New superblock for explicit mount
        const FS_CONTEXT_FOR_MOUNT = 0;
        /// New superblock for automatic submount
        const FS_CONTEXT_FOR_SUBMOUNT = 1;
        /// Superblock reconfiguration (remount)
        const FS_CONTEXT_FOR_RECONFIGURE = 2;
    }
}

atomic_bitflags!(FsopenFlags, AtomicU32);
atomic_bitflags!(FsConfigCmd, AtomicU32);
atomic_bitflags!(FsmountFlags, AtomicU32);
atomic_bitflags!(FsContextPhase, AtomicU8);
atomic_bitflags!(FsContextPurpose, AtomicU8);
