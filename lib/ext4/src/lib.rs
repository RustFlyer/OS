#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

pub mod dentry;
pub mod disk;
pub mod file;
pub mod fs;
pub mod inode;
pub mod superblock;

mod ext;

extern crate alloc;
