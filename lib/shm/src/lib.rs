#![no_std]
extern crate alloc;

use alloc::{sync::Weak, vec::Vec};
use config::mm::PAGE_SIZE;
use id::ShmStat;
use mm::page_cache::page::Page;

pub mod flags;
pub mod id;
pub mod manager;

pub struct SharedMemory {
    pub stat: ShmStat,
    pub pages: Vec<Weak<Page>>,
}

impl SharedMemory {
    pub fn new(sz: usize, pid: usize) -> Self {
        Self {
            stat: ShmStat::new(sz, pid),
            pages: Vec::with_capacity(sz / PAGE_SIZE + 1),
        }
    }
    pub fn size(&self) -> usize {
        self.stat.segsz
    }
}
