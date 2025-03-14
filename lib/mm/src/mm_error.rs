//! Module for errors related to memory management.

use core::fmt::{self, Display};

/// Alloc error.
#[derive(Debug)]
pub enum AllocError {
    /// Out of free frames.
    OutOfMemory,
}

impl Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AllocError::OutOfMemory => write!(f, "Out of memory"),
        }
    }
}
