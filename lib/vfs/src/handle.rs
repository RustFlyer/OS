use core::ffi::CStr;

use alloc::{string::{String, ToString}, vec::Vec};

use systype::error::{SysError, SysResult};

pub const HANDLE_LEN: usize = 128;

/// Internal representation of a Linux `file_handle` structure.
///
/// The external representation is defined as:
/// ```c
/// struct file_handle {
///     u32 handle_bytes; // size of the handle field in bytes
///     i32 handle_type;  // type of the handle
///     char handle[128]; // path to the file, null-terminated
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileHandle {
    header: FileHandleHeader,
    data: FileHandleData,
}

impl FileHandle {
    /// Creates a new `FileHandle` with the specified handle type and file path.
    ///
    /// This function is crate-private; external code should call [`Dentry::file_handle`]
    /// to create a [`FileHandle`] instead.
    pub(crate) fn new(handle_type: i32, path: String) -> Self {
        FileHandle {
            header: FileHandleHeader {
                handle_bytes: HANDLE_LEN as u32,
                handle_type,
            },
            data: FileHandleData { path },
        }
    }

    pub fn handle_bytes(&self) -> u32 {
        self.header.handle_bytes
    }

    pub fn handle_type(&self) -> i32 {
        self.header.handle_type
    }

    pub fn path(&self) -> &String {
        &self.data.path
    }

    /// Converts the `FileHandle` to Linux `file_handle` structure as a byte vector.
    pub fn to_raw_bytes(&self) -> Vec<u8> {
        debug_assert!(self.data.path.len() < HANDLE_LEN, "Path length exceeds HANDLE_LEN bytes");

        let mut bytes = vec![0; 8 + HANDLE_LEN];
        bytes[0..4].copy_from_slice(&self.header.handle_bytes.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.header.handle_type.to_ne_bytes());
        bytes[8..8 + self.data.path.len()].copy_from_slice(self.data.path.as_bytes());
        bytes[8 + self.data.path.len()] = 0; // Null-terminate
        bytes
    }
}

/// This structure corresponds the `handle_bytes` and `handle_type` fields of the Linux
/// `file_handle` structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FileHandleHeader {
    handle_bytes: u32,
    handle_type: i32,
}

impl FileHandleHeader {
    pub fn handle_bytes(&self) -> u32 {
        self.handle_bytes
    }

    pub fn set_handle_bytes(&mut self, handle_bytes: u32) {
        self.handle_bytes = handle_bytes;
    }

    pub fn handle_type(&self) -> i32 {
        self.handle_type
    }

    /// Converts the `handle_bytes` and `handle_type` fields of the Linux `file_handle`
    /// structure as bytes to a [`FileHandleHeader`].
    ///
    /// `bytes` must be 8 bytes long.
    pub fn from_raw_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == 8);

        let handle_bytes = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let handle_type = i32::from_ne_bytes(bytes[4..8].try_into().unwrap());
        FileHandleHeader {
            handle_bytes,
            handle_type,
        }
    }
}

/// This structure corresponds to the `handle` field of the Linux `file_handle`
/// structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileHandleData {
    path: String,
}

impl FileHandleData {
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Converts the `handle` field of the Linux `file_handle` structure as bytes to a
    /// [`FileHandleData`].
    pub fn from_raw_bytes(bytes: &[u8]) -> SysResult<Self> {
        let cstr = CStr::from_bytes_until_nul(bytes)
            .map_err(|_| SysError::EINVAL)?;
        let path = cstr.to_str()
            .map_err(|_| SysError::EINVAL)?
            .to_string();
        Ok(FileHandleData { path })
    }
}
