#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]
#![feature(ptr_as_ref_unchecked)]

pub mod dentry;
pub mod disk;
pub mod file;
pub mod fs;
pub mod inode;
pub mod superblock;

mod ext;

extern crate alloc;
