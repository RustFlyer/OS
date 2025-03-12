#![no_std]
#![feature(negative_impls)]
#![feature(sync_unsafe_cell)]
pub mod mutex;

pub use mutex::*;
