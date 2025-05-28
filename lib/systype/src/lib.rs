//! This crate is a derivative work based on the original code from the Phoenix OS project.
//! The original code is licensed under MIT License.
//! The original code can be found at https://github.com/djphoenix/phoenix-os.

#![no_std]

extern crate alloc;

pub mod error;
pub mod memory_flags;
pub mod rlimit;
pub mod rusage;
pub mod time;
