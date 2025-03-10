#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

pub struct IdAllocator {
    current_id: usize,
    ids: Vec<usize>,
}

impl IdAllocator {
    pub fn new() -> Self {
        IdAllocator {
            current_id: 0,
            ids: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> usize {
        if self.ids.is_empty() {
            let id = self.current_id;
            self.current_id += 1;
            id as usize
        } else {
            self.ids.pop().unwrap() as usize
        }
    }

    pub fn dealloc(&mut self, id: usize) {
        self.ids.push(id);
    }
}
