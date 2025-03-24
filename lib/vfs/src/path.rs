extern crate alloc;
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use systype::{SysError, SysResult};

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

    /// walk the path to return the final dentry
    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if path.starts_with("/") {
            self.root.clone()
        } else {
            self.start.clone()
        };
        let nodes: Vec<&str> = path
            .split("/")
            .filter(|name| !name.is_empty() && *name != ".")
            .collect();
        for node in nodes {
            match node {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => match dentry.lookup(name) {
                    Ok(child_dentry) => dentry = child_dentry,
                    Err(e) => return Err(e),
                },
            }
        }
        Ok(dentry)
    }
}
