#![no_std]
#![feature(alloc_error_handler)]
#![feature(btree_cursors)]
#![feature(ptr_as_ref_unchecked)]
#![feature(sync_unsafe_cell)]

pub mod address;
pub mod frame;
pub mod heap;
pub mod vm;

#[macro_use]
extern crate alloc;
