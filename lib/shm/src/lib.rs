#![no_std]

use alloc::{sync::Arc, vec::Vec};

use config::mm::PAGE_SIZE;
use id::ShmStat;
use mm::page_cache::page::Page;
pub mod flags;
pub mod id;
pub mod manager;

#[macro_use]
extern crate alloc;

#[derive(Debug)]
pub struct SharedMemory {
    pub stat: ShmStat,
    pub pages: Vec<Option<Arc<Page>>>,
}

impl SharedMemory {
    pub fn new(size: usize, pid: usize) -> Self {
        let size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        Self {
            stat: ShmStat::new(size, pid),
            pages: vec![None; size / PAGE_SIZE],
        }
    }
    pub fn size(&self) -> usize {
        self.stat.segsz
    }
}
