use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsopenFlags: u32 {
        /// Close-on-exec flag for the returned file descriptor
        const FSOPEN_CLOEXEC = 0x80000;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FsConfigCmd: u32 {
        /// Set a string parameter
        const FSCONFIG_SET_STRING = 0;
        /// Set a string parameter with val specified as a string
        const FSCONFIG_SET_BINARY = 1;
        /// Set a parameter using a path
        const FSCONFIG_SET_PATH = 2;
        /// Set a parameter using a path, but with AT_EMPTY_PATH
        const FSCONFIG_SET_PATH_EMPTY = 3;
        /// Set a parameter using a file descriptor
        const FSCONFIG_SET_FD = 5;
        /// Clear a parameter flag
        const FSCONFIG_SET_FLAG = 6;
        /// Create the superblock
        const FSCONFIG_CMD_CREATE = 7;
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
