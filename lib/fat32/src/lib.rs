#![no_std]

extern crate alloc;

use fatfs::{Dir, DirIter, Error, File, FileSystem, LossyOemCpConverter, NullTimeProvider};

use disk::DiskCursor;
use systype::error::SysError;

pub mod dentry;
pub mod disk;
pub mod file;
pub mod fs;
pub mod inode;
pub mod superblock;

type FatDir = Dir<'static, DiskCursor, NullTimeProvider, LossyOemCpConverter>;
type FatFile = File<'static, DiskCursor, NullTimeProvider, LossyOemCpConverter>;
type FatDirIter = DirIter<'static, DiskCursor, NullTimeProvider, LossyOemCpConverter>;
type FatFs = FileSystem<DiskCursor>;

pub const fn as_sys_err(err: fatfs::Error<()>) -> SysError {
    match err {
        Error::NotFound => SysError::ENOENT,
        _ => SysError::EIO,
    }
}
