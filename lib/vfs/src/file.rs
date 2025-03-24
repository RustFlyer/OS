extern crate alloc;

use core::sync::atomic::AtomicUsize;

use crate::{dentry::Dentry, inode::Inode};
use alloc::sync::Arc;

pub struct FileMeta {
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    pub pos: AtomicUsize,
}

pub trait File {
    fn get_meta(&self) -> FileMeta;
}
