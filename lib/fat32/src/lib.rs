#![no_main]
#![no_std]

use disk::DiskCursor;
use fatfs::{Dir, DirIter, Error, File, FileSystem, LossyOemCpConverter, NullTimeProvider};
use systype::SysError;

extern crate alloc;

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

pub const fn as_sys_err(err: fatfs::Error<()>) -> systype::SysError {
    match err {
        Error::NotFound => SysError::ENOENT,
        _ => SysError::EIO,
    }
}
