#![no_std]
#![no_main]
#![feature(ptr_as_ref_unchecked)]
#![feature(sync_unsafe_cell)]
#![feature(assert_matches)]

use alloc::sync::Arc;
use dentry::Dentry;
use spin::Once;

pub mod dcache;
pub mod dentry;
pub mod direntry;
pub mod fanotify;
pub mod file;
pub mod fstype;
pub mod handle;
pub mod inode;
pub mod inoid;
pub mod kstat;
pub mod path;
pub mod stat;
pub mod superblock;

#[macro_use]
extern crate alloc;

pub static SYS_ROOT_DENTRY: Once<Arc<dyn Dentry>> = Once::new();

pub fn sys_root_dentry() -> Arc<dyn Dentry> {
    SYS_ROOT_DENTRY.get().unwrap().clone()
}
