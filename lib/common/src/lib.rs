#![no_std]
#![allow(unused)]

extern crate alloc;

pub mod atomicflags;
pub mod ringbuffer;

use alloc::string::String;
pub use ringbuffer::*;

static mulfs: [&str; 6] = [
    "mkfs.ext3",
    "mkfs.ext4",
    "mkfs.exfat",
    "mkfs.bcachefs",
    "mkfs.btrfs",
    "mkfs.xfs",
];

pub fn test_more_fs(mut path: String) -> String {
    for fs in mulfs {
        path = path.replace(fs, "mkfs.ext2");
    }

    path
}
