//! This module provides data structures, traits, and constants for fanotify support in
//! the VFS.

use alloc::{
    alloc::{Layout, alloc, dealloc},
    slice,
    vec::Vec,
};
use core::{mem, ptr};

use bitflags::bitflags;

use super::constants::*;

/// The `fanotify_event_metadata` structure in Linux.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FanotifyEventMetadata {
    pub event_len: u32,
    pub vers: u8,
    pub reserved: u8,
    pub metadata_len: u16,
    pub mask: FanEvent,
    pub fd: i32,
    pub pid: i32,
}

/// This structure holds the `fanotify_event_info_fid` structure in Linux.
///
/// The original structure ends with a variable-length array `handle`, which is not
/// allowed in Rust. Instead, we manually manage the memory using raw pointers.
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
#[derive(Debug, Clone, Copy)]
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

impl Default for FanotifyEventInfoFid {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FanotifyEventInfoPid {
    pub hdr: FanotifyEventInfoHeader,
    pub pidfd: i32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FanotifyEventInfoError {
    pub hdr: FanotifyEventInfoHeader,
    pub error: i32,
    pub error_count: u32,
}

// TODO: check the `Default` implementation.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct FanotifyEventInfoHeader {
    pub info_type: u8,
    pub pad: u8,
    pub len: u16,
}

bitflags! {
    /// Mask for fanotify events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanEvent: u64 {
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
    }
}
