use alloc::{
    string::{String, ToString},
    sync::Arc,
};

use config::inode::InodeType;
use systype::{SysError, SysResult};

use crate::{dentry::Dentry, sys_root_dentry};

/// A struct representing a path in the filesystem.
#[derive(Clone)]
pub struct Path {
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
    /// Creates a path from a starting dentry and a path string.
    ///
    /// `start` is a dentry that serves as the current working directory, from which
    /// the path will be resolved in case the path is relative. It is ignored if the
    /// path is absolute.
    ///
    /// `path` is a string representing the path. If it starts with a `/`, it is
    /// resolved as an absolute path. Otherwise, it is resolved as a relative path
    /// from the `start` dentry.
    ///
    /// This function does not check the validity of the path. If there are illegal
    /// characters in `path`, or if `path` is empty, the behavior is undefined. The
    /// caller must ensure that `path` is a valid path string.
    pub fn new(start: Arc<dyn Dentry>, path: String) -> Self {
        debug_assert!(!path.is_empty());
        debug_assert!(!path.contains('\0'));
        Self { start, path }
    }

    /// Walks the path to find the target dentry.
    ///
    /// Returns a valid dentry if the target file exists. Returns an invalid dentry
    /// if the target file does not exist but its parent directory does. Returns an
    /// `ENOENT` error if any directory in the middle of the path does not exist.
    ///
    /// For example, if the file tree is:
    /// ```.
    /// /
    /// ├── a
    /// │   └── b
    /// └── c
    /// ```
    /// - `walk("/a/b")` returns a valid dentry for `/a/b`.
    /// - `walk("/a/x")` returns an invalid dentry for `/a/x`.
    /// - `walk("/x")` returns an invalid dentry for `/x`.
    /// - `walk("/x/y")` returns an `ENOENT` error.
    ///
    /// Other errors may be returned if it encounters other issues.
    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();

        let mut dentry = if path.starts_with("/") {
            Arc::clone(&sys_root_dentry())
        } else {
            Arc::clone(&self.start)
        };
        for name in path
            .split("/")
            .filter(|name| !name.is_empty() && *name != ".")
        {
            if dentry.is_negative() {
                return Err(SysError::ENOENT);
            }
            match name {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => {
                    dentry = dentry.lookup(name)?;
                }
            }
        }
        Ok(dentry)
    }

    pub fn resolve_dentry(mut dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn Dentry>> {
        const MAX_DEPTH: usize = 40;
        for _ in 0..MAX_DEPTH {
            if dentry.is_negative() {
                return Ok(dentry);
            }
            match dentry.inode().unwrap().inotype() {
                InodeType::SymLink => {
                    let mut target_path_buf: [u8; 64] = [0; 64];
                    let _r = dentry.clone().base_open()?.readlink(&mut target_path_buf)?;
                    let target_path = core::str::from_utf8_mut(&mut target_path_buf)
                        .map_err(|_| SysError::EINVAL)?;

                    let parent = dentry.parent().ok_or(SysError::ENOENT)?;
                    let base = if target_path.starts_with('/') {
                        sys_root_dentry()
                    } else {
                        parent
                    };
                    let path = Path::new(base, target_path.to_string());
                    dentry = path.walk()?;
                }
                _ => return Ok(dentry),
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
