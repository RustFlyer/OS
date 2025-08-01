//! This module provides data structures, traits, and constants for fanotify support in
//! the VFS.

use alloc::{
    alloc::{Layout, alloc, dealloc},
    slice,
};
use core::ptr;

use bitflags::bitflags;

use config::vfs::OpenFlags;

use super::constants::*;

/// The `fanotify_event_metadata` structure in Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyEventMetadata {
    pub event_len: u32,
    pub vers: u8,
    pub reserved: u8,
    pub metadata_len: u16,
    pub mask: FanEventMask,
    pub fd: i32,
    pub pid: i32,
}

/// This structure holds the `fanotify_event_info_fid` structure in Linux.
///
/// The original structure ends with a variable-length array `handle`, which is not
/// allowed in Rust. Instead, we manually manage the memory. The pointer `inner` points
/// to a [`FanotifyEventInfoFidInner`] structure, followed by zero or more bytes which
/// contain the `handle` field.
///
/// The user should access the fields via methods provided by this struct, and get a
/// Linux `fanotify_event_info_fid` struct as a byte array via the `as_bytes()` method.
#[derive(Debug)]
pub struct FanotifyEventInfoFid {
    memory: *mut u8,
    size: usize,
}

/// The `fanotify_event_info_fid` structure in Linux, without the `handle` field.
///
/// The `handle` field is a variable-length array, so it is not included in this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyEventInfoFidInner {
    hdr: FanotifyEventInfoHeader,
    fsid: FsId,
}

pub type FsId = [i32; 2];

impl FanotifyEventInfoFid {
    /// Creates an empty `FanotifyEventInfoFid` instance.
    pub fn new(handle_len: usize) -> Self {
        let size = size_of::<FanotifyEventInfoFidInner>() + handle_len;
        let align = align_of::<FanotifyEventInfoFidInner>();
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        let memory = unsafe { alloc(layout) };
        unsafe {
            ptr::write_bytes(memory, 0, size);
        }
        Self { memory, size }
    }

    pub fn hdr(&self) -> FanotifyEventInfoHeader {
        self.as_inner().hdr
    }

    pub fn set_hdr(&mut self, hdr: FanotifyEventInfoHeader) {
        self.as_inner_mut().hdr = hdr;
    }

    pub fn fsid(&self) -> FsId {
        self.as_inner().fsid
    }

    pub fn set_fsid(&mut self, fsid: FsId) {
        self.as_inner_mut().fsid = fsid;
    }

    pub fn handle(&self) -> &[u8] {
        let handle_ptr = unsafe { self.memory.add(size_of::<FanotifyEventInfoFidInner>()) };
        let handle_len = self.size - size_of::<FanotifyEventInfoFidInner>();
        unsafe { slice::from_raw_parts(handle_ptr, handle_len) }
    }

    pub fn handle_mut(&mut self) -> &mut [u8] {
        let handle_ptr = unsafe { self.memory.add(size_of::<FanotifyEventInfoFidInner>()) };
        let handle_len = self.size - size_of::<FanotifyEventInfoFidInner>();
        unsafe { slice::from_raw_parts_mut(handle_ptr, handle_len) }
    }

    /// Returns the `fanotify_event_info_fid` structure it holds as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.memory, self.size) }
    }

    fn as_inner(&self) -> &FanotifyEventInfoFidInner {
        unsafe { &*(self.memory as *const FanotifyEventInfoFidInner) }
    }

    fn as_inner_mut(&mut self) -> &mut FanotifyEventInfoFidInner {
        unsafe { &mut *(self.memory as *mut FanotifyEventInfoFidInner) }
    }
}

impl Drop for FanotifyEventInfoFid {
    fn drop(&mut self) {
        if !self.memory.is_null() {
            let size = self.size;
            let align = align_of::<FanotifyEventInfoFidInner>();
            let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
            unsafe { dealloc(self.memory, layout) };
        }
    }
}

unsafe impl Send for FanotifyEventInfoFid {}
unsafe impl Sync for FanotifyEventInfoFid {}

impl Clone for FanotifyEventInfoFid {
    fn clone(&self) -> Self {
        let size = self.size;
        let align = align_of::<FanotifyEventInfoFidInner>();
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        let inner = unsafe { alloc(layout) };
        unsafe {
            ptr::copy_nonoverlapping(self.memory, inner, size);
        }
        Self {
            memory: inner,
            size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyEventInfoPid {
    pub hdr: FanotifyEventInfoHeader,
    pub pidfd: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyEventInfoError {
    pub hdr: FanotifyEventInfoHeader,
    pub error: i32,
    pub error_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyEventInfoHeader {
    pub info_type: FanotifyEventInfoType,
    pub pad: u8,
    pub len: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FanotifyEventInfoType {
    Fid = FAN_EVENT_INFO_TYPE_FID,
    Dfid = FAN_EVENT_INFO_TYPE_DFID,
    DfidName = FAN_EVENT_INFO_TYPE_DFID_NAME,
    OldDfidName = FAN_EVENT_INFO_TYPE_OLD_DFID_NAME,
    NewDfidName = FAN_EVENT_INFO_TYPE_NEW_DFID_NAME,
    PidFd = FAN_EVENT_INFO_TYPE_PIDFD,
    Error = FAN_EVENT_INFO_TYPE_ERROR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FanotifyResponse {
    pub fd: i32,
    pub response: FanotifyResponseOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum FanotifyResponseOption {
    Allow = FAN_ALLOW,
    Deny = FAN_DENY,
}

bitflags! {
    /// Mask for fanotify events. It contains several event bits and three flags bits;
    /// each event bit corresponds to a specific fanotify event, and each flag bit
    /// specifies a special behavior of monitoring.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanEventMask: u64 {
        // Each of the following constants is a bit corresponding to a fanotify
        // notification event.

        /// A file or a directory was accessed (read).
        const ACCESS = FAN_ACCESS;

        /// A file was modified.
        const MODIFY = FAN_MODIFY;

        /// A file or directory was opened for writing (`O_WRONLY` or `O_RDWR`) was
        /// closed.
        const CLOSE_WRITE = FAN_CLOSE_WRITE;

        /// A file or directory that was opened read-only (`O_RDONLY`) was closed.
        const CLOSE_NOWRITE = FAN_CLOSE_NOWRITE;

        /// A file or a directory was opened.
        const OPEN = FAN_OPEN;

        /// A file or a directory was opened with the intent to be executed.
        const OPEN_EXEC = FAN_OPEN_EXEC;

        // The following notification events require the fanotify group to identify
        // filesystem objects by file handles.

        /// A file or a directory metadata was changed.
        const ATTRIB = FAN_ATTRIB;

        /// A child file or directory was created in a watched parent.
        const CREATE = FAN_CREATE;

        /// A child file or directory was deleted in a watched parent.
        const DELETE = FAN_DELETE;

        /// A watched file or directory was deleted.
        const DELETE_SELF = FAN_DELETE_SELF;

        /// A filesystem error was detected.
        const FS_ERROR = FAN_FS_ERROR;

        /// A file or directory has been moved from a watched parent directory.
        const MOVED_FROM = FAN_MOVED_FROM;

        /// A file or directory has been moved to a watched parent directory.
        const MOVED_TO = FAN_MOVED_TO;

        /// A file or directory has been moved to or from a watched parent directory.
        const RENAME = FAN_RENAME;

        /// A watched file or directory was modified.
        const MOVE_SELF = FAN_MOVE_SELF;

        // Each of the following constants is a bit corresponding to a fanotify permission
        // event. They require the fanotify group both to identify filesystem objects by
        // file handles, and to be initialized with `FAN_CLASS_CONTENT` or
        // `FAN_CLASS_PRE_CONTENT`.

        /// An application wants to read a file or directory, for example using `read`
        /// or `readdir`. The reader must write a response that determines whether the
        /// permission to access the filesystem object shall be granted.
        const ACCESS_PERM = FAN_ACCESS_PERM;

        /// An application wants to open a file or directory. The reader must write a
        /// response that determines whether the permission to open the filesystem
        /// object shall be granted.
        const OPEN_PERM = FAN_OPEN_PERM;

        /// An application wants to open a file for execution. The reader must write a
        /// response that determines whether the permission to open the filesystem object
        /// for execution shall be granted.
        const OPEN_EXEC_PERM = FAN_OPEN_EXEC_PERM;

        // The following two masks are used to check for specific event types.

        /// This bit mask is used to check for any close event.
        const CLOSE = FAN_CLOSE;

        /// This bit mask is used to check for any move event.
        const MOVE = FAN_MOVE;

        // The following three constants are flags that modify the behavior of monitoring.

        /// The events described in the mask have occurred on a directory object.
        /// Reporting events on directories requires setting this flag in the mark mask.
        /// It is reported in an event mask only if the fanotify group identifies
        /// filesystem objects by file handles.
        const ONDIR = FAN_ONDIR;

        /// Events for the immediate children of marked directories shall be created.
        const EVENT_ON_CHILD = FAN_EVENT_ON_CHILD;

        /// This flag indicates that the event queue has exceeded the limit on the number
        /// of events. This limit can be overridden by specifying the
        /// `FAN_UNLIMITED_QUEUE` flag when calling `fanotify_init`.
        const Q_OVERFLOW = FAN_Q_OVERFLOW;

        /// This bit mask is used to check whether an event is a file event. If an event
        /// is not a file event, it is a directory event. Note that flags `ONDIR`,
        /// `EVENT_ON_CHILD`, and `Q_OVERFLOW` are not included in this mask, so the
        /// user should be careful when using this mask to check for file events.
        const FILE_EVENT_MASK =
            Self::ACCESS.bits()
          | Self::MODIFY.bits()
          | Self::CLOSE_WRITE.bits()
          | Self::CLOSE_NOWRITE.bits()
          | Self::OPEN.bits()
          | Self::OPEN_EXEC.bits()
          | Self::ATTRIB.bits()
          | Self::DELETE_SELF.bits()
          | Self::FS_ERROR.bits()
          | Self::MOVE_SELF.bits()
          | Self::ACCESS_PERM.bits()
          | Self::OPEN_PERM.bits()
          | Self::OPEN_EXEC_PERM.bits();

        /// This bit mask is used to check whether an event is a directory event. If an
        /// event is not a directory event, it is a file event. Note that flags `ONDIR`,
        /// `EVENT_ON_CHILD`, and `Q_OVERFLOW` are not included in this mask, so the
        /// user should be careful when using this mask to check for directory events.
        const DIR_EVENT_MASK =
            Self::CREATE.bits()
          | Self::DELETE.bits()
          | Self::MOVED_FROM.bits()
          | Self::MOVED_TO.bits()
          | Self::RENAME.bits();

        /// This bit mask contains all events that need the group to identify filesystem
        /// objects by file handles.
        const FID_EVENT_MASK =
            Self::ATTRIB.bits()
          | Self::CREATE.bits()
          | Self::DELETE.bits()
          | Self::DELETE_SELF.bits()
          | Self::FS_ERROR.bits()
          | Self::MOVED_FROM.bits()
          | Self::MOVED_TO.bits()
          | Self::RENAME.bits()
          | Self::MOVE_SELF.bits();
    }
}

bitflags! {
    /// Flags for defining the behavior of the fanotify group to be initialized via the
    /// `fanotify_init` system call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanInitFlags: u32 {
        const CLASS_PRE_CONTENT = FAN_CLASS_PRE_CONTENT;
        const CLASS_CONTENT = FAN_CLASS_CONTENT;
        const CLASS_NOTIF = FAN_CLASS_NOTIF;

        const CLOEXEC = FAN_CLOEXEC;
        const NONBLOCK = FAN_NONBLOCK;
        const UNLIMITED_QUEUE = FAN_UNLIMITED_QUEUE;
        const UNLIMITED_MARKS = FAN_UNLIMITED_MARKS;
        const REPORT_TID = FAN_REPORT_TID;
        const ENABLE_AUDIT = FAN_ENABLE_AUDIT;
        const REPORT_FID = FAN_REPORT_FID;
        const REPORT_DIR_FID = FAN_REPORT_DIR_FID;
        const REPORT_NAME = FAN_REPORT_NAME;
        const REPORT_TARGET_FID = FAN_REPORT_TARGET_FID;
        const REPORT_DFID_NAME_TARGET = FAN_REPORT_DFID_NAME_TARGET;
        const REPORT_PIDFD = FAN_REPORT_PIDFD;
    }
}

impl From<FanInitFlags> for OpenFlags {
    fn from(flags: FanInitFlags) -> Self {
        let mut open_flags = Self::O_RDWR;
        if flags.contains(FanInitFlags::CLOEXEC) {
            open_flags |= OpenFlags::O_CLOEXEC;
        }
        if flags.contains(FanInitFlags::NONBLOCK) {
            open_flags |= OpenFlags::O_NONBLOCK;
        }
        open_flags
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FanEventFileFlags: u32 {
        const RDONLY = OpenFlags::O_RDONLY.bits() as u32;
        const WRONLY = OpenFlags::O_WRONLY.bits() as u32;
        const RDWR = OpenFlags::O_RDWR.bits() as u32;
        const LARGEFILE = OpenFlags::O_LARGEFILE.bits() as u32;
        const CLOEXEC = OpenFlags::O_CLOEXEC.bits() as u32;
        const APPEND = OpenFlags::O_APPEND.bits() as u32;
        const DSYNC = OpenFlags::O_DSYNC.bits() as u32;
        const NOATIME = OpenFlags::O_NOATIME.bits() as u32;
        const NONBLOCK = OpenFlags::O_NONBLOCK.bits() as u32;
        const SYNC = OpenFlags::O_SYNC.bits() as u32;
    }
}

impl From<OpenFlags> for FanEventFileFlags {
    fn from(flags: OpenFlags) -> Self {
        Self::from_bits_truncate(flags.bits() as u32)
    }
}

impl From<FanEventFileFlags> for OpenFlags {
    fn from(flags: FanEventFileFlags) -> Self {
        Self::from_bits_retain(flags.bits() as i32)
    }
}

bitflags! {
    /// Flags for modifying fanotify marks via the `fanotify_mark` system call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanMarkFlags: u32 {
        /// Add a mark to the fanotify group.
        const ADD = FAN_MARK_ADD;

        /// Remove a mark from the fanotify group.
        const REMOVE = FAN_MARK_REMOVE;

        /// Remove either all marks for filesystems, all marks for mounts, or all marks
        /// for directories and files from the fanotify group.
        ///
        /// This can be used in conjunction only with either `FAN_MARK_FILESYSTEM` or
        /// `FAN_MARK_MOUNT`, which means that all marks for the specified filesystem or
        /// mount will be removed. Otherwise, no other flags can be used with this flag,
        /// and `fanotify_mark` will remove all marks for directories and files.
        const FLUSH = FAN_MARK_FLUSH;

        /// Mark the symbolic link itself, rather than the file it refers to.
        const DONT_FOLLOW = FAN_MARK_DONT_FOLLOW;

        /// Mark only directories. If the filesystem object to be marked is not a
        /// directory, `fanotify_mark` will return `ENOTDIR`.
        const ONLYDIR = FAN_MARK_ONLYDIR;

        /// Mark the mount specified by `pathname`. If `pathname` is not a mount point,
        /// the mount containing it will be marked. All directories, subdirectories, and
        /// the contained files of the mount will be monitored.
        const MOUNT = FAN_MARK_MOUNT;

        const FILESYSTEM = FAN_MARK_FILESYSTEM;

        /// The events in mask shall be added to or removed from the ignore mask.
        const IGNORED_MASK = FAN_MARK_IGNORED_MASK;

        const IGNORE = FAN_MARK_IGNORE;

        const IGNORED_SURV_MODIFY = FAN_MARK_IGNORED_SURV_MODIFY;

        const EVICTABLE = FAN_MARK_EVICTABLE;
    }
}

impl FanotifyResponse {
    /// Creates a FanotifyResponse from a byte buffer.
    ///
    /// The buffer should contain exactly 8 bytes. If the buffer is not of the correct
    /// size or contains an invalid response value, this function returns `None`.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() != 8 {
            return None;
        }

        let fd = i32::from_ne_bytes(buf[0..4].try_into().unwrap());
        let response_val = u32::from_ne_bytes(buf[4..8].try_into().unwrap());

        let response = match response_val {
            FAN_ALLOW => FanotifyResponseOption::Allow,
            FAN_DENY => FanotifyResponseOption::Deny,
            _ => return None,
        };

        Some(FanotifyResponse { fd, response })
    }

    /// Returns true if this response allows the operation.
    pub fn is_allow(&self) -> bool {
        self.response == FanotifyResponseOption::Allow
    }

    /// Returns true if this response denies the operation.
    pub fn is_deny(&self) -> bool {
        self.response == FanotifyResponseOption::Deny
    }
}
