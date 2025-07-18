//! This module provides data structures, traits, and constants for fanotify support in
//! the VFS.

use alloc::{
    alloc::{Layout, alloc, dealloc},
    slice,
    sync::Arc,
};
use core::{mem, ptr};

use bitflags::bitflags;

use config::vfs::OpenFlags;

use crate::file::File;

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
/// allowed in Rust. Instead, we manually manage the memory.
///
/// The user should access the fields via methods provided by this struct, and get a
/// Linux `fanotify_event_info_fid` struct as a byte array via the `as_bytes()` method.
#[derive(Debug)]
pub struct FanotifyEventInfoFid {
    inner: *mut u8,
    size: usize,
}

/// The `fanotify_event_info_fid` structure in Linux, without the `handle` field.
///
/// The `handle` field is a variable-length array, so it is not included in this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
struct FanotifyEventInfoFidInner {
    hdr: FanotifyEventInfoHeader,
    fsid: FsId,
}

type FsId = [i32; 2];

impl FanotifyEventInfoFid {
    /// Creates an `FanotifyEventInfoFid` instance with an empty `handle` field.
    pub fn new() -> Self {
        let size = size_of::<FanotifyEventInfoFidInner>();
        let align = align_of::<FanotifyEventInfoFidInner>();
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        let inner = unsafe { alloc(layout) };
        unsafe {
            *inner = Default::default();
        }
        Self { inner, size }
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
        let handle_ptr = unsafe { self.inner.add(size_of::<FanotifyEventInfoFidInner>()) };
        let handle_len = self.size - size_of::<FanotifyEventInfoFidInner>();
        unsafe { slice::from_raw_parts(handle_ptr, handle_len) }
    }

    pub fn handle_mut(&mut self) -> &mut [u8] {
        let handle_ptr = unsafe { self.inner.add(size_of::<FanotifyEventInfoFidInner>()) };
        let handle_len = self.size - size_of::<FanotifyEventInfoFidInner>();
        unsafe { slice::from_raw_parts_mut(handle_ptr, handle_len) }
    }

    /// Returns the `fanotify_event_info_fid` structure it holds as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.inner, self.size) }
    }

    fn as_inner(&self) -> &FanotifyEventInfoFidInner {
        unsafe { &*(self.inner as *const FanotifyEventInfoFidInner) }
    }

    fn as_inner_mut(&mut self) -> &mut FanotifyEventInfoFidInner {
        unsafe { &mut *(self.inner as *mut FanotifyEventInfoFidInner) }
    }
}

impl Drop for FanotifyEventInfoFid {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let size = self.size;
            let align = align_of::<FanotifyEventInfoFidInner>();
            let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
            unsafe { dealloc(self.inner, layout) };
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
            ptr::copy_nonoverlapping(self.inner, inner, size);
        }
        Self { inner, size }
    }
}

impl Default for FanotifyEventInfoFid {
    fn default() -> Self {
        Self::new()
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
    pub info_type: u8,
    pub pad: u8,
    pub len: u16,
}

/// Enum representing an fanotify metadata structure or an information record structure.
#[derive(Clone)]
pub enum FanotifyEventData {
    /// Fanotify event metadata. The first element is an uncomplete metadata. The second
    /// element is an open file to the monitored filesystem object, which is to be added
    /// to the process's file descriptor table when the event is read.
    Metadata((FanotifyEventMetadata, Arc<dyn File>)),
    Info(FanotifyEventInfoFid),
    Pid(FanotifyEventInfoPid),
    Error(FanotifyEventInfoError),
}

impl FanotifyEventData {
    /// Returns a byte slice representation of the data.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            FanotifyEventData::Metadata(metadata) => unsafe {
                slice::from_raw_parts(
                    &raw const metadata.0 as *const u8,
                    mem::size_of::<FanotifyEventMetadata>(),
                )
            },
            FanotifyEventData::Info(info) => info.as_bytes(),
            FanotifyEventData::Pid(pid) => unsafe {
                slice::from_raw_parts(
                    pid as *const FanotifyEventInfoPid as *const u8,
                    mem::size_of::<FanotifyEventInfoPid>(),
                )
            },
            FanotifyEventData::Error(error) => unsafe {
                slice::from_raw_parts(
                    error as *const FanotifyEventInfoError as *const u8,
                    mem::size_of::<FanotifyEventInfoError>(),
                )
            },
        }
    }
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
    /// Mask for fanotify events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanEventMask: u64 {
        // Each of the following constants is a bit corresponding to a fanotify
        // notification event.

        /// A file or a directory was accessed (read).
        const ACCESS = FAN_ACCESS;

        /// A file or a directory was opened.
        const OPEN = FAN_OPEN;

        /// A file or a directory was opened with the intent to be executed.
        const OPEN_EXEC = FAN_OPEN_EXEC;

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

        /// A file or directory has been moved to or from a watched parent directory.
        const RENAME = FAN_RENAME;

        /// A file or directory has been moved from a watched parent directory.
        const MOVED_FROM = FAN_MOVED_FROM;

        /// A file or directory has been moved to a watched parent directory.
        const MOVED_TO = FAN_MOVED_TO;

        /// A watched file or directory was modified.
        const MOVE_SELF = FAN_MOVE_SELF;

        /// A file was modified.
        const MODIFY = FAN_MODIFY;

        /// A file or directory was opened for writing (`O_WRONLY` or `O_RDWR`) was
        /// closed.
        const CLOSE_WRITE = FAN_CLOSE_WRITE;

        /// A file or directory that was opened read-only (`O_RDONLY`) was closed.
        const CLOSE_NOWRITE = FAN_CLOSE_NOWRITE;

        /// The event queue exceeded the limit on number of events. This limit can be
        /// overridden by specifying the `FAN_UNLIMITED_QUEUE` flag when calling
        /// `fanotify_init`.
        const Q_OVERFLOW = FAN_Q_OVERFLOW;

        // Each of the following constants is a bit corresponding to a fanotify
        // permission event.

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

        /// The events described in the mask have occurred on a directory object.
        /// Reporting events on directories requires setting this flag in the mark mask.
        /// It is reported in an event mask only if the fanotify group identifies
        /// filesystem objects by file handles.
        const ONDIR = FAN_ONDIR;

        /// Events for the immediate children of marked directories shall be created.
        const EVENT_ON_CHILD = FAN_EVENT_ON_CHILD;
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
        const REPORT_FD = FAN_REPORT_PIDFD;
        const REPORT_DIR_FID = FAN_REPORT_DIR_FID;
        const REPORT_NAME = FAN_REPORT_NAME;
        const REPORT_TARGET_FID = FAN_REPORT_TARGET_FID;
        const REPORT_DFID_NAME_TARGET = FAN_REPORT_DFID_NAME_TARGET;
        const REPORT_PIDFD = FAN_REPORT_PIDFD;
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
