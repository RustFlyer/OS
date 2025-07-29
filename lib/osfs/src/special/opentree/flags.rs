use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct OpenTreeFlags: u32 {
        /// Make the resulting descriptor close-on-exec
        const OPEN_TREE_CLOEXEC = 0x400000; // O_CLOEXEC
        /// Create a detached mount tree instead of opening location
        const OPEN_TREE_CLONE = 0x1;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct MountFlags: u32 {
        /// Mount read-only
        const MS_RDONLY = 1;
        /// Ignore suid and sgid bits
        const MS_NOSUID = 2;
        /// Disallow access to device special files
        const MS_NODEV = 4;
        /// Disallow program execution
        const MS_NOEXEC = 8;
        /// Writes are synced at once
        const MS_SYNCHRONOUS = 16;
        /// Alter flags of existing mount
        const MS_REMOUNT = 32;
        /// Allow mandatory locks on filesystem
        const MS_MANDLOCK = 64;
        /// Directory modifications are synchronous
        const MS_DIRSYNC = 128;
        /// Do not update access times
        const MS_NOATIME = 1024;
        /// Do not update directory access times
        const MS_NODIRATIME = 2048;
        /// Bind directory at different place
        const MS_BIND = 4096;
        /// Move a subtree
        const MS_MOVE = 8192;
        /// Recursively apply to all mounts in subtree
        const MS_REC = 16384;
        /// Make mount private
        const MS_PRIVATE = 1 << 18;
        /// Make mount shared
        const MS_SHARED = 1 << 20;
        /// Make mount slave
        const MS_SLAVE = 1 << 19;
        /// Make mount unbindable
        const MS_UNBINDABLE = 1 << 17;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct MountAttrFlags: u64 {
        /// Mount read-only
        const MOUNT_ATTR_RDONLY = 0x00000001;
        /// Ignore suid and sgid bits
        const MOUNT_ATTR_NOSUID = 0x00000002;
        /// Disallow access to device special files
        const MOUNT_ATTR_NODEV = 0x00000004;
        /// Disallow program execution
        const MOUNT_ATTR_NOEXEC = 0x00000008;
        /// Access time update behavior mask
        const MOUNT_ATTR__ATIME = 0x00000070;
        /// Always update access time
        const MOUNT_ATTR_RELATIME = 0x00000000;
        /// Do not update access times
        const MOUNT_ATTR_NOATIME = 0x00000010;
        /// Always update access time
        const MOUNT_ATTR_STRICTATIME = 0x00000020;
        /// Do not update directory access times
        const MOUNT_ATTR_NODIRATIME = 0x00000080;
        /// Create ID-mapped mount
        const MOUNT_ATTR_IDMAP = 0x00100000;
        /// Do not follow symlinks
        const MOUNT_ATTR_NOSYMFOLLOW = 0x00200000;
    }
}

atomic_bitflags!(OpenTreeFlags, AtomicU32);
atomic_bitflags!(MountFlags, AtomicU32);
atomic_bitflags!(MountAttrFlags, AtomicU64);
