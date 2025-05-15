//! Trap handlers for different architectures

#![allow(unused)]

mod user_trap_handler;
mod kernel_trap_handler;

pub use user_trap_handler::*;
pub use kernel_trap_handler::*;
