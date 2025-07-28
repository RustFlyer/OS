use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct InotifyFlags: u32 {
        /// Create file descriptor with close-on-exec flag set
        const IN_CLOEXEC    = 0x80000;
        /// Create file descriptor with non-blocking flag set
        const IN_NONBLOCK   = 0x800;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct InotifyMask: u32 {
        /// File was accessed
        const IN_ACCESS        = 0x00000001;
        /// File was modified
        const IN_MODIFY        = 0x00000002;
        /// Metadata changed
        const IN_ATTRIB        = 0x00000004;
        /// Writtable file was closed
        const IN_CLOSE_WRITE   = 0x00000008;
        /// Unwrittable file was closed
        const IN_CLOSE_NOWRITE = 0x00000010;
        /// File was opened
        const IN_OPEN          = 0x00000020;
        /// File was moved from X
        const IN_MOVED_FROM    = 0x00000040;
        /// File was moved to Y
        const IN_MOVED_TO      = 0x00000080;
        /// Subfile was created
        const IN_CREATE        = 0x00000100;
        /// Subfile was deleted
        const IN_DELETE        = 0x00000200;
        /// Self was deleted
        const IN_DELETE_SELF   = 0x00000400;
        /// Self was moved
        const IN_MOVE_SELF     = 0x00000800;

        /// All events which a program can wait on
        const IN_ALL_EVENTS    = Self::IN_ACCESS.bits() | Self::IN_MODIFY.bits() |
                                 Self::IN_ATTRIB.bits() | Self::IN_CLOSE_WRITE.bits() |
                                 Self::IN_CLOSE_NOWRITE.bits() | Self::IN_OPEN.bits() |
                                 Self::IN_MOVED_FROM.bits() | Self::IN_MOVED_TO.bits() |
                                 Self::IN_CREATE.bits() | Self::IN_DELETE.bits() |
                                 Self::IN_DELETE_SELF.bits() | Self::IN_MOVE_SELF.bits();

        /// Close events
        const IN_CLOSE         = Self::IN_CLOSE_WRITE.bits() | Self::IN_CLOSE_NOWRITE.bits();
        /// Move events
        const IN_MOVE          = Self::IN_MOVED_FROM.bits() | Self::IN_MOVED_TO.bits();

        /// Only watch the path if it is a directory
        const IN_ONLYDIR       = 0x01000000;
        /// Don't follow a sym link
        const IN_DONT_FOLLOW   = 0x02000000;
        /// Exclude events on unlinked children
        const IN_EXCL_UNLINK   = 0x04000000;
        /// Add to the mask of an already existing watch
        const IN_MASK_ADD      = 0x20000000;
        /// Event occurred against dir
        const IN_ISDIR         = 0x40000000;
        /// Only send event once
        const IN_ONESHOT       = 0x80000000;

        /// Ignored event
        const IN_IGNORED       = 0x00008000;
        /// Queue overflowed
        const IN_Q_OVERFLOW    = 0x00004000;
        /// File system was unmounted
        const IN_UNMOUNT       = 0x00002000;
    }
}

atomic_bitflags!(InotifyFlags, AtomicU32);
atomic_bitflags!(InotifyMask, AtomicU32);
