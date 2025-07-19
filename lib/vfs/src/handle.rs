use alloc::{string::String, vec::Vec};

use systype::error::{SysError, SysResult};

/// Internal representation of a Linux `file_handle` structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileHandle {
    header: FileHandleHeader,
    data: FileHandleData,
}

impl FileHandle {
    /// Creates a new `FileHandle` with the specified handle type and file path.
    pub fn new(handle_type: i32, path: String) -> Self {
        FileHandle {
            header: FileHandleHeader {
                handle_bytes: path.len() as u32,
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
        let mut bytes = Vec::with_capacity(8 + self.header.handle_bytes as usize);
        bytes.extend_from_slice(&self.header.handle_bytes.to_ne_bytes());
        bytes.extend_from_slice(&self.header.handle_type.to_ne_bytes());
        bytes.extend_from_slice(self.data.path.as_bytes());
        bytes
    }
}

/// This structure corresponds the `handle_bytes` and `handle_type` fields of the Linux
/// `file_handle` structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct FileHandleHeader {
    /// The number of bytes in the handle, excluding the `handle_bytes` and `handle_type`
    /// fields. This field is properly initialized when the [`FileHandle`] is created via
    /// [`FileHandle::new`], and is set arbitrarily when it is converted from a Linux
    /// `file_handle` structure via [`FileHandle::from_linux_file_handle`].
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
        let path = String::from_utf8(bytes.to_vec()).or(Err(SysError::EINVAL))?;
        Ok(FileHandleData { path })
    }
}
