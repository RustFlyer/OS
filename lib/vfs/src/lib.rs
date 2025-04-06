#![no_std]
#![no_main]
#![feature(ptr_as_ref_unchecked)]
#![feature(sync_unsafe_cell)]

pub mod dcache;
pub mod dentry;
pub mod direntry;
pub mod file;
pub mod fstype;
pub mod inode;
pub mod inoid;
pub mod kstat;
pub mod path;
pub mod superblock;

#[macro_use]
extern crate alloc;
