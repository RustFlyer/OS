//! Module for wrapping `ext4_file` struct from `lwext4_rust` crate, which represents
//! a file in the ext4 filesystem that is not a directory.
//!
//! This module provides functions to open, close, read, write, truncate, and seek file,
//! along with other file operations.

use alloc::{ffi::CString, string::String};
use core::{cell::SyncUnsafeCell, ffi::CStr, mem::MaybeUninit};

use lwext4_rust::bindings::{
    SEEK_CUR, SEEK_END, SEEK_SET, ext4_fclose, ext4_file, ext4_flink, ext4_fopen, ext4_fopen2,
    ext4_fread, ext4_fremove, ext4_frename, ext4_fs_get_inode_ref, ext4_fs_put_inode_ref,
    ext4_fseek, ext4_fsize, ext4_fsymlink, ext4_ftell, ext4_ftruncate, ext4_fwrite, ext4_inode,
    ext4_inode_ref, ext4_readlink,
};

use config::vfs::{OpenFlags, SeekFrom};
use systype::error::{SysError, SysResult};

use crate::ext::{dir::ExtDir, inode::ext4_inode_get_size};

/// Wrapper for `lwext4_rust` crate's `ext4_file` struct.
pub struct ExtFile(
    // Some functions in `lwext4_rust` crate require `&mut ext4_file` as a parameter,
    // even though the `ext4_file` is actually not mutated. This is a bug in the
    // `lwext4_rust` crate (which automatically generates the bindings from C code).
    // To fix this, we use `SyncUnsafeCell` here, so that we can get a mutable pointer
    // from a shared reference to an `ExtFile`, and pass the mutable pointer to functions
    // in `lwext4_rust` that do not mutate the `ext4_file` struct. However, be careful
    // that we should not get a mutable pointer from a shared reference by calling
    // `SyncUnsafeCell::get()` and pass the mutable pointer to a function that indedd
    // mutates the `ext4_file` struct, which violates Rust's borrowing rules.
    //
    // Note: You should construct an `ExtFile` by calling `ExtFile::open()`; do not
    // construct it directly as `ExtFile(SyncUnsafeCell::new(ext4_file))`, because
    // this does not really open the file. Only do this if you know the `ext4_file` is
    // already opened, and make sure you close it later correctly.
    pub(crate) SyncUnsafeCell<ext4_file>,
);

// Note that `ext4_file` contains a raw mutable pointer, so it is not `Send` or `Sync`.
// We mark `ExtFile` as `Send` and `Sync`, but we have not checked if it is safe to do so.
unsafe impl Send for ExtFile {}
unsafe impl Sync for ExtFile {}

impl Drop for ExtFile {
    fn drop(&mut self) {
        unsafe {
            // Note: `ext4_fclose` has a return value, so it may be inappropriate to call
            // it in a destructor. We should check what `ext4_fclose` does and decide
            // whether to ignore the return value or handle it properly.
            ext4_fclose(self.0.get_mut());
        }
    }
}

impl ExtFile {
    /// This function does the same as [`Self::open2`], except that it takes a string
    /// as the `flags` parameter instead of an [`OpenFlags`] enum.
    ///
    /// The `flags` string is mapped to the corresponding [`OpenFlags`] value as follows:
    /// - `r` or `rb`: `O_RDONLY`
    /// - `w` or `wb`: `O_WRONLY | O_CREAT | O_TRUNC`
    /// - `a` or `ab`: `O_WRONLY | O_CREAT | O_APPEND`
    /// - `r+`, `rb+`, or `r+b`: `O_RDWR`
    /// - `w+`, `wb+`, or `w+b`: `O_RDWR | O_CREAT | O_TRUNC`
    /// - `a+`, `ab+`, or `a+b`: `O_RDWR | O_CREAT | O_APPEND`
    pub(crate) fn open(path: &CStr, flags: String) -> SysResult<Self> {
        let mut file: MaybeUninit<ext4_file> = MaybeUninit::uninit();
        let err = unsafe {
            ext4_fopen(
                file.as_mut_ptr(),
                path.as_ptr(),
                flags.as_ptr() as *const ::core::ffi::c_char,
            )
        };
        match err {
            0 => unsafe { Ok(Self(SyncUnsafeCell::new(file.assume_init()))) },
            e => {
                let err = SysError::from_i32(e);
                log::warn!(
                    "ext4_fopen failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Opens a file at the given path with the specified flags and returns a handle to it.
    ///
    /// `path` is the absolute path to the file to be opened.
    /// `flags` are the flags used to open the file.
    pub(crate) fn open2(path: &CStr, flags: OpenFlags) -> SysResult<Self> {
        let mut file: MaybeUninit<ext4_file> = MaybeUninit::uninit();
        let err = unsafe { ext4_fopen2(file.as_mut_ptr(), path.as_ptr(), flags.bits()) };
        match err {
            0 => unsafe { Ok(Self(SyncUnsafeCell::new(file.assume_init()))) },
            e => {
                let err = SysError::from_i32(e);
                log::warn!(
                    "ext4_fopen failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    pub(crate) fn open_by_ino(mount_point: &str, ino: u32, flags: OpenFlags) -> SysResult<Self> {
        let mountpoint = {
            let mp_path = CString::new(mount_point).unwrap();
            let mp_dir = ExtDir::open(mp_path.as_c_str()).unwrap();
            unsafe { mp_dir.0.f.mp.as_mut_unchecked() }
        };
        let filesystem = &mut mountpoint.fs;

        let mut inode_ref = {
            let mut inode_ref: MaybeUninit<ext4_inode_ref> = MaybeUninit::uninit();
            let err = unsafe { ext4_fs_get_inode_ref(filesystem, ino, inode_ref.as_mut_ptr()) };
            match err {
                0 => unsafe { inode_ref.assume_init() },
                e => {
                    let err = SysError::from_i32(e);
                    log::warn!(
                        "ext4_fs_get_inode_ref failed: ino = {}, error = {:?}",
                        ino,
                        err
                    );
                    return Err(err);
                }
            }
        };

        let superblock = &mut filesystem.sb;
        let inode = unsafe { inode_ref.inode.as_mut_unchecked() };
        let file_size = unsafe { ext4_inode_get_size(superblock, inode) };

        let file = ext4_file {
            mp: mountpoint,
            inode: ino,
            flags: flags.bits() as u32,
            fsize: file_size,
            fpos: 0,
        };

        unsafe {
            ext4_fs_put_inode_ref(&mut inode_ref);
        }

        Ok(Self(SyncUnsafeCell::new(file)))
    }

    /// Reads data from the file into the provided buffer. This function will try to
    /// read `buf.len()` bytes into `buf`, but it may read fewer bytes if it reaches EOF.
    /// This function will advance the file offset.
    ///
    /// Returns the number of bytes read.
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> SysResult<usize> {
        let mut count = 0;
        let err = unsafe {
            ext4_fread(
                self.0.get_mut(),
                buf.as_mut_ptr() as _,
                buf.len(),
                &mut count,
            )
        };
        match err {
            0 => Ok(count),
            e => {
                let err = SysError::from_i32(e);
                log::warn!("ext4_fread failed: ino {}, error = {:?}", self.ino(), err);
                Err(err)
            }
        }
    }

    /// Writes data from the provided buffer to the file. This function will try to
    /// write `buf.len()` bytes from `buf` to the file, but it may write fewer bytes
    /// if there is not enough space. This function will advance the file offset.
    ///
    /// Returns the number of bytes written.
    pub(crate) fn write(&mut self, buf: &[u8]) -> SysResult<usize> {
        let mut count = 0;
        let err =
            unsafe { ext4_fwrite(self.0.get_mut(), buf.as_ptr() as _, buf.len(), &mut count) };
        match err {
            0 => Ok(count),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_fwrite failed: ino = {}, error = {:?}",
                    self.ino(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Truncates the file to the specified size.
    ///
    /// This function will change the size of the file to `size` bytes. If the file size
    /// is larger than `size`, the extra data will be discarded. If the file size is
    /// smaller than `size`, the file will be padded with zeros.
    pub(crate) fn truncate(&mut self, size: u64) -> SysResult<()> {
        let err = unsafe { ext4_ftruncate(self.0.get_mut(), size) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_ftruncate failed: ino = {}, error = {:?}",
                    self.ino(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Seeks to a specific position in the file.
    ///
    /// `offset` is the number of bytes to seek. `seek_type` specifies how to interpret the
    /// offset:
    /// - `SeekSet`: Seek from the beginning of the file.
    /// - `SeekCur`: Seek from the current position in the file.
    /// - `SeekEnd`: Seek from the end of the file.
    pub(crate) fn seek(&mut self, seek: SeekFrom) -> SysResult<()> {
        let offset = match seek {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => offset,
            SeekFrom::End(offset) => offset,
        };
        let seek_type = SeekType::from(seek);
        let err = unsafe { ext4_fseek(self.0.get_mut(), offset, seek_type as u32) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!("ext4_fseek failed: ino = {}, error = {:?}", self.ino(), err);
                Err(err)
            }
        }
    }

    /// Returns the current position in the file.
    pub(crate) fn tell(&self) -> u64 {
        unsafe { ext4_ftell(self.0.get()) }
    }

    /// Returns the size of the file in bytes.
    pub(crate) fn size(&self) -> u64 {
        unsafe { ext4_fsize(self.0.get()) }
    }

    pub(crate) fn setsize(&self) -> u64 {
        0
    }

    /* I'm not sure if we need the following two functions. Maybe remove it later. */

    /// Returns the inode number of the file.
    pub(crate) fn ino(&self) -> u32 {
        unsafe { (*self.0.get()).inode }
    }

    /// Returns the open flags of the file.
    pub(crate) fn flags(&self) -> OpenFlags {
        // We do an `unwrap` here because we don't know if the translation from
        // `flags` to `OpenFlags` is always valid. If the kernel panics here
        // during development, we should rewrite this function.
        unsafe { OpenFlags::from_bits((*self.0.get()).flags as i32).unwrap() }
    }

    /// Creates a hard link named `path` to the file `target`.
    pub(crate) fn link(target: &CStr, path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_flink(target.as_ptr(), path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_flink failed: target = {}, path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    target.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Change the name or location of a file.
    pub(crate) fn rename(path: &CStr, new_path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_frename(path.as_ptr(), new_path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_flink failed: path = {}, new_path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    new_path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Unlinks a file at the given path.
    pub(crate) fn unlink(path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_fremove(path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_fremove failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Creates a symbolic link named `path` which points to the file `target`.
    pub(crate) fn symlink(target: &CStr, path: &CStr) -> SysResult<()> {
        let err = unsafe { ext4_fsymlink(target.as_ptr(), path.as_ptr()) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_flink failed: target = {}, path = {}, error = {:?}",
                    target.to_str().unwrap(),
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Reads the contents of a symbolic link at the given path into the provided buffer.
    ///
    /// Returns the number of bytes read.
    pub(crate) fn readlink(path: &CStr, buf: &mut [u8]) -> SysResult<usize> {
        let mut count = 0;
        let err =
            unsafe { ext4_readlink(path.as_ptr(), buf.as_mut_ptr() as _, buf.len(), &mut count) };
        match err {
            0 => Ok(count),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_readlink failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }
}

/// Enumeration for file seek types.
/// This is used in [`ExtFile::seek`] to specify how to interpret the seeking offset.
#[repr(u32)]
enum SeekType {
    Set = SEEK_SET,
    Cur = SEEK_CUR,
    End = SEEK_END,
}

impl From<SeekFrom> for SeekType {
    fn from(seek_from: SeekFrom) -> Self {
        match seek_from {
            SeekFrom::Start(_) => SeekType::Set,
            SeekFrom::Current(_) => SeekType::Cur,
            SeekFrom::End(_) => SeekType::End,
        }
    }
}
