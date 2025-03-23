extern crate alloc;
use alloc::{
    string::{String, ToString},
    sync::Arc,
};

use crate::dentry::Dentry;

#[derive(Clone)]
pub struct Path {
    root: Arc<dyn Dentry>,
    start: Arc<dyn Dentry>,
    path: String,
}

impl Eq for Path {}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && Arc::ptr_eq(&self.start, &other.start)
    }
}

impl Path {
    pub fn new(root: Arc<dyn Dentry>, start: Arc<dyn Dentry>, path: &str) -> Self {
        Self {
            root,
            start,
            path: path.to_string(),
        }
    }
}
