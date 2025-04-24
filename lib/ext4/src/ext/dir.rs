//! Module for wrapping `ext4_dir` struct from `lwext4_rust` crate, which represents a
//! directory file in the ext4 filesystem.
//!
//! This module provides functions to open, create, and remove directories, and read
//! directory entries, along with other directory-related operations.

use alloc::string::String;
use core::{
    cell::SyncUnsafeCell,
    ffi::CStr,
    mem::{ManuallyDrop, MaybeUninit},
    panic,
};

use lwext4_rust::{
    InodeTypes,
    bindings::{
        ext4_dir, ext4_dir_close, ext4_dir_entry_next, ext4_dir_entry_rewind, ext4_dir_mk,
        ext4_dir_mv, ext4_dir_open, ext4_direntry,
    },
};

use config::inode::InodeType;
use systype::{SysError, SysResult};

use super::file::ExtFile;

/// Wrapper for `lwext4_rust` crate's `ext4_dir` struct which represents a directory
/// file which can reads and writes directory entries.
pub struct ExtDir(ext4_dir);

/// Wrapper for `lwext4_rust` crate's `ext4_direntry` struct which represents a directory
/// entry.
///
/// This struct wraps a shared reference to `ext4_direntry`, which points into a
/// [`ExtDir`] instance. The user can get an [`ExtDirEntry`] by calling
/// [`ExtDir::next()`], and Rust's borrow checker will ensure that the [`ExtDirEntry`]
/// is valid.
pub struct ExtDirEntry<'a>(&'a ext4_direntry);

impl Drop for ExtDir {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_close(&mut self.0);
        }
    }
}

impl ExtDir {
    /// Returns an [`ExtFile`] which is a handle to the directory file to allow
    /// file operations on it.
    ///
    /// The returned [`ExtFile`] is wrapped in a [`ManuallyDrop`]; do not drop it, e.g.,
    /// by calling [`ManuallyDrop::drop`]! Also be careful when calling functions like
    /// [`ExtFile::write`] or [`ExtFile::truncate`] on the returned [`ExtFile`]; this is
    /// usually not what you want to do.
    pub fn as_file(&self) -> ManuallyDrop<ExtFile> {
        ManuallyDrop::new(ExtFile(SyncUnsafeCell::new(self.0.f)))
    }

    /// Opens a directory file at the given path and returns a handle to it.
    ///
    /// `path` is the absolute path to the file to be opened.
    pub fn open(path: &CStr) -> SysResult<Self> {
        let mut dir = MaybeUninit::uninit();
        let err = unsafe { ext4_dir_open(dir.as_mut_ptr(), path.as_ptr()) };
        match err {
            0 => unsafe { Ok(Self(dir.assume_init())) },
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_dir_open failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Creates a directory at the given path.
    pub fn create(path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_dir_mk(path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_dir_mk failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Returns a shared reference to the next directory entry in the directory.
    /// Returns `None` if there are no more entries.
    pub fn next(&mut self) -> Option<ExtDirEntry> {
        unsafe { ext4_dir_entry_next(&mut self.0).as_ref() }.map(ExtDirEntry)
    }

    /// Rewinds the directory entry offset to the beginning of the directory file. When
    /// calling [`Self::next()`] after this, it will return directory entries from the
    /// beginning again.
    pub fn rewind(&mut self) {
        unsafe {
            ext4_dir_entry_rewind(&mut self.0);
        }
    }

    /// Recursively removes a directory and all its contents.
    pub fn remove_recursively(path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_dir_mk(path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_dir_rm failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Change the name or location of a directory.
    pub fn rename(path: &CStr, new_path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_dir_mv(path.as_ptr(), new_path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_dir_mv failed: old_path = {}, new_path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    new_path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }
}

impl ExtDirEntry<'_> {
    /// Returns the inode number of the directory entry.
    pub fn ino(&self) -> u32 {
        self.0.inode
    }

    /// Returns the inode type of the directory entry.
    pub fn file_type(&self) -> InodeType {
        match InodeTypes::from(self.0.inode_type as usize) {
            InodeTypes::EXT4_DE_BLKDEV => InodeType::BlockDevice,
            InodeTypes::EXT4_DE_CHRDEV => InodeType::CharDevice,
            InodeTypes::EXT4_DE_DIR => InodeType::Dir,
            InodeTypes::EXT4_DE_FIFO => InodeType::Fifo,
            InodeTypes::EXT4_DE_SYMLINK => InodeType::SymLink,
            InodeTypes::EXT4_DE_REG_FILE => InodeType::File,
            InodeTypes::EXT4_DE_SOCK => InodeType::Socket,
            InodeTypes::EXT4_DE_UNKNOWN => InodeType::Unknown,
            // `InodeTypes` enum in `lwext4_rust` crate is badly designed; some variants
            // are duplicated and we just ignore them here.
            _ => panic!(),
        }
    }

    /// Returns the name of the directory entry.
    pub fn name(&self) -> SysResult<String> {
        let name_bytes = self.0.name[..self.0.name_length as usize].to_vec();
        String::from_utf8(name_bytes).map_err(|_| SysError::EUTFFAIL)
    }
}
