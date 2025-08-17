use alloc::slice;
use core::mem;

use systype::error::{SysError, SysResult};

pub const HEADER_LEN: usize = mem::size_of::<FileHandleHeader>();
pub const HANDLE_LEN: usize = mem::size_of::<FileHandleData>();
pub const FILE_HANDLE_LEN: usize = HEADER_LEN + HANDLE_LEN;

/// Internal representation of a Linux `file_handle` structure.
///
/// The external representation is defined as:
/// ```c
/// struct file_handle {
///     unsigned handle_bytes; // size of the handle field in bytes
///     int handle_type;       // type of the handle
///     struct {
///         uint32_t inode;         // inode number
///         uint32_t generation;    // generation number
///     } handle;              // handle data
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FileHandle {
    header: FileHandleHeader,
    data: FileHandleData,
}

impl FileHandle {
    /// Creates a new `FileHandle` with the specified handle type and file path.
    ///
    /// This function is crate-private; external code should call [`Dentry::file_handle`]
    /// to create a [`FileHandle`] instead.
    pub(crate) fn new(handle_type: u32, inode: u32, generation: u32) -> Self {
        FileHandle {
            header: FileHandleHeader {
                handle_bytes: HANDLE_LEN as u32,
                handle_type,
            },
            data: FileHandleData { inode, generation },
        }
    }

    pub fn handle_bytes(&self) -> u32 {
        self.header.handle_bytes
    }

    pub fn handle_type(&self) -> u32 {
        self.header.handle_type
    }

    pub fn inode(&self) -> u32 {
        self.data.inode
    }

    /// Converts a byte representation of a Linux `file_handle` structure to a
    /// [`FileHandle`].
    ///
    /// This function is intended for debugging purposes. It should not be called to parse
    /// a file handle passed from a user program, as the caller does not know the length
    /// of the file handle beforehand. Instead, call [`FileHandleHeader::from_raw_bytes`]
    /// first to get the length of the handle, and then call
    /// [`FileHandleData::from_raw_bytes`] with the remaining bytes.
    pub fn from_raw_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == FILE_HANDLE_LEN);

        let header = FileHandleHeader::from_raw_bytes(&bytes[0..8]);
        let data = FileHandleData::from_raw_bytes(&bytes[8..]).unwrap();

        FileHandle { header, data }
    }

    /// Converts the `FileHandle` to Linux `file_handle` structure as a byte slice.
    pub fn as_raw_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const FileHandle as *const u8, FILE_HANDLE_LEN) }
    }
}

/// This structure corresponds the `handle_bytes` and `handle_type` fields of the Linux
/// `file_handle` structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FileHandleHeader {
    pub handle_bytes: u32,
    pub handle_type: u32,
}

impl FileHandleHeader {
    /// Converts the `handle_bytes` and `handle_type` fields of the Linux `file_handle`
    /// structure as bytes to a [`FileHandleHeader`].
    ///
    /// `bytes` must be 8 bytes long.
    pub fn from_raw_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == 8);

        let handle_bytes = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let handle_type = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());

        FileHandleHeader {
            handle_bytes,
            handle_type,
        }
    }
}

/// This structure corresponds to the `handle` field of the Linux `file_handle`
/// structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FileHandleData {
    /// Inode number of the file.
    pub inode: u32,
    /// Generation number of the file.
    pub generation: u32,
}

impl FileHandleData {
    /// Converts the `handle` field of the Linux `file_handle` structure as bytes to a
    /// [`FileHandleData`].
    ///
    /// `bytes` must be properly sized to match the `handle` field, or this function will
    /// return `EINVAL`.
    pub fn from_raw_bytes(bytes: &[u8]) -> SysResult<Self> {
        if bytes.len() != mem::size_of::<FileHandleData>() {
            return Err(SysError::EINVAL);
        }
        let inode = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let generation = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
        Ok(FileHandleData { inode, generation })
    }
}
