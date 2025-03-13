#![no_std]

extern crate alloc;

use alloc::vec::Vec;

/// A trait for types that can allocate and deallocate `usize` IDs.
///
/// This trait is used to abstract over different ID allocation strategies.
pub trait IdAllocator {
    /// Allocate a new ID.
    ///
    /// Returns `Some(id)` if an ID was successfully allocated,
    /// or `None` if no more IDs are available.
    fn alloc(&mut self) -> Option<usize>;

    /// Deallocate an ID.
    ///
    /// This allows the ID to be reused in future calls to `alloc`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the ID was previously allocated
    /// and has not been deallocated yet.
    unsafe fn dealloc(&mut self, id: usize);
}

/// An ID allocator that uses a simple vector to store recycled IDs.
///
/// This allocator is very fast but takes up a lot of memory.
pub struct VecIdAllocator {
    next: usize,
    end: usize,
    ids: Vec<usize>,
}

impl VecIdAllocator {
    /// Create a new `VecAllocator` that can allocate IDs in the range `from..to`.
    ///
    /// # Panics
    ///
    /// Panics if `from >= to`.
    pub fn new(from: usize, to: usize) -> Self {
        debug_assert!(from < to);
        VecIdAllocator {
            next: from,
            end: to,
            ids: Vec::new(),
        }
    }
}

impl IdAllocator for VecIdAllocator {
    fn alloc(&mut self) -> Option<usize> {
        match self.ids.pop() {
            Some(id) => Some(id),
            None => {
                let id = self.next;
                if id < self.end {
                    self.next += 1;
                    Some(id)
                } else {
                    None
                }
            }
        }
    }

    unsafe fn dealloc(&mut self, id: usize) {
        self.ids.push(id);
    }
}
