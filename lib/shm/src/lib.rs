#![no_std]
extern crate alloc;

use id::ShmIdDs;
use alloc::{sync::Weak, vec::Vec};
use mm::page_cache::page::Page;
use config::mm::PAGE_SIZE;

pub mod id;
pub mod manager;


pub struct SharedMemory {
    pub shmid_ds: ShmIdDs,
    pub pages: Vec<Weak<Page>>,
}

impl SharedMemory {
    pub fn new(sz: usize, pid: usize) -> Self {
        Self {
            shmid_ds: ShmIdDs::new(sz, pid),
            pages: Vec::with_capacity(sz / PAGE_SIZE + 1),
        }
    }
    pub fn size(&self) -> usize {
        self.shmid_ds.shm_segsz
    }
}