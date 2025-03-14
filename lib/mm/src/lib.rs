#![no_std]
#![feature(alloc_error_handler)]

pub mod address;
pub mod frame;
pub mod heap;
pub mod vm;

#[macro_use]
extern crate alloc;

pub use vm::page_table::enable_kernel_page_table;
