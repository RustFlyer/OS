use alloc::{string::String, sync::Arc};
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
            path: String::from(path),
        }
    }

    /// walk the path to return the final dentry
    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if path.starts_with("/") {
            Arc::clone(&self.root)
        } else {
            Arc::clone(&self.start)
        };
        for name in path
            .split("/")
            .filter(|name| !name.is_empty() && *name != ".")
        {
            match name {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => dentry = dentry.lookup(name)?,
            }
        }
        Ok(dentry)
    }
}
