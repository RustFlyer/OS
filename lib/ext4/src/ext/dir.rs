use alloc::ffi::CString;
use core::mem::MaybeUninit;

use lwext4_rust::bindings::{
    ext4_dir, ext4_dir_close, ext4_dir_entry_next, ext4_dir_entry_rewind, ext4_dir_mk,
    ext4_dir_open,
};

/// Wrapper for C-interface `ext4_dir` struct which represents a directory.
pub struct ExtDir(ext4_dir);

/// A directory entry in Ext4 filesystem.
pub struct ExtDirEntry<'a> {
    pub inode: u32,
    pub name: &'a str,
    pub type_: u8,
}

impl Drop for ExtDir {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_close(&mut self.0);
        }
    }
}

impl ExtDir {
    /// Opens a directory at the given path and returns a handle to it.
    pub fn open(path: &str) -> Result<Self, i32> {
        let c_path = CString::new(path).unwrap();
        let mut dir = MaybeUninit::uninit();
        let err = unsafe { ext4_dir_open(dir.as_mut_ptr(), c_path.as_ptr()) };

        match err {
            0 => unsafe { Ok(Self(dir.assume_init())) },
            e => {
                log::warn!("ext4_dir_open failed: {}, error = {}", path, err);
                Err(e)
            }
        }
    }

    /// Creates a new directory at the given path, opens it, and returns a handle to it.
    pub fn create(path: &str) -> Result<Self, i32> {
        let c_path = CString::new(path).unwrap();
        let err = unsafe { ext4_dir_mk(c_path.as_ptr()) };
        if err != 0 {
            log::warn!("ext4_dir_mk failed: {}, error = {}", path, err);
            return Err(err);
        }
        ExtDir::open(path)
    }

    /// Returns the next directory entry in the directory.
    /// Returns `None` if there are no more entries.
    pub fn next(&mut self) -> Option<ExtDirEntry> {
        unsafe {
            if ext4_dir_entry_next(&mut self.0).is_null() {
                return None;
            }
        };
        let name = &self.0.de.name[..self.0.de.name_length as usize];
        Some(ExtDirEntry {
            inode: self.0.de.inode,
            name: core::str::from_utf8(&name).unwrap(),
            type_: self.0.de.inode_type,
        })
    }

    /// Rewinds the directory to the beginning. When calling `next()` after this,
    /// it will return directory entries from the beginning again.
    pub fn rewind(&mut self) {
        unsafe {
            ext4_dir_entry_rewind(&mut self.0);
        }
    }
}
