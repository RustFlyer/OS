use alloc::{string::String, sync::Arc};
use config::inode::InodeType;
use systype::{SysError, SysResult};

use crate::{dentry::Dentry, sys_root_dentry};

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
        log::info!("dentry path: {}", path);

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
                name => {
                    dentry = dentry.lookup(name)?;
                }
            }
        }
        log::info!("dentry: {}", dentry.path());
        Ok(dentry)
    }

    pub fn resolve_dentry(dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn Dentry>> {
        const MAX_DEPTH: usize = 40;
        let mut current_dentry = dentry;
        for _ in 0..MAX_DEPTH {
            if current_dentry.is_negative() {
                return Ok(current_dentry);
            }
            match current_dentry.inode().ok_or(SysError::ENOENT)?.inotype() {
                InodeType::SymLink => {
                    let mut target_path_buf: [u8; 64] = [0; 64];
                    let _r = current_dentry
                        .clone()
                        .base_open()?
                        .readlink(&mut target_path_buf)?;
                    let target_path = core::str::from_utf8_mut(&mut target_path_buf)
                        .map_err(|_| SysError::EINVAL)?;

                    let parent = current_dentry.parent().ok_or(SysError::ENOENT)?;
                    let base = if target_path.starts_with('/') {
                        sys_root_dentry()
                    } else {
                        parent
                    };
                    let path = Path::new(sys_root_dentry(), base, &target_path);
                    current_dentry = path.walk()?;
                }
                _ => return Ok(current_dentry),
            }
        }
        Err(SysError::ELOOP)
    }
}

pub fn split_parent_and_name(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
