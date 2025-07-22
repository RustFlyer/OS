//! Module for abstracting paths in the filesystem.
//!
//! This module provides a [`Path`] struct that represents a path in the filesystem
//! and provides methods to resolve it to a dentry. The user can create a [`Path`]
//! from a path string, and then call [`Path::walk`] to resolve it to a dentry.
//! Symlinks are supported and the user can decide whether to resolve them or not.

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use config::inode::InodeType;
use systype::error::{SysError, SysResult};

use crate::{dentry::Dentry, file::File, sys_root_dentry};

/// A struct representing a path in the filesystem which can be resolved to a
/// dentry.
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
    /// See [`Self::walk`] for more details on how the path is resolved and what errors
    /// may be returned. Note that `start` may be a negative dentry or not a directory,
    /// in which case corresponding errors will be returned when calling [`Self::walk`].
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
    /// Returns a valid dentry if the target file exists.
    /// Returns an invalid dentry if the target file does not exist but its parent
    /// directory does.
    /// Returns an `ENOENT` error if any directory in the middle of the path does not
    /// exist.
    /// Returns an `ENOTDIR` error if any directory in the middle of the path is not a
    /// directory.
    /// Returns an `ELOOP` error if it encounters too many symlinks.
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
    ///
    /// Note that this function will resolve symlinks in the middle of the path.
    /// However, if the target file itself is a symlink, it will not be resolved.
    /// The caller may want to call [`Self::resolve_symlink`] on the returned dentry
    /// to resolve it.
    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        self.walk_recursive(&mut 0, None)
    }

    /// similar to walk, the different function is getting parent dentrys
    pub fn walk_with_parents(
        &self,
        dentry_list: &mut Vec<Arc<dyn Dentry>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        self.walk_recursive(&mut 0, Some(dentry_list))
    }

    /// Do the same as [`Self::walk`], but with a counter to help to limit the
    /// recursion depth.
    fn walk_recursive(
        &self,
        counter: &mut usize,
        dentry_list: Option<&mut Vec<Arc<dyn Dentry>>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();

        let list_exist = dentry_list.is_some();
        let mut list: Vec<Arc<dyn Dentry>> = Vec::new();

        let mut dentry = if path.starts_with("/") {
            Arc::clone(&sys_root_dentry())
        } else {
            Arc::clone(&self.start)
        };
        for name in path
            .split("/")
            .filter(|name| !name.is_empty() && *name != ".")
        {
            loop {
                if dentry.is_negative() {
                    return Err(SysError::ENOENT);
                }
                let inode_type = dentry.inode().unwrap().inotype();
                if inode_type == InodeType::SymLink {
                    // log::debug!("[walk_recursive] read SymLink {}", dentry.path());
                    dentry = Self::resolve_symlink_recursive(Arc::clone(&dentry), counter)?;
                } else if inode_type == InodeType::Dir {
                    break;
                } else {
                    return Err(SysError::ENOTDIR);
                }
            }
            match name {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => {
                    // log::debug!("[walk_recursive] {} try to look up {}", dentry.path(), name);
                    dentry = dentry.lookup(name)?;
                }
            }

            if list_exist {
                list.push(dentry.clone());
            }
        }

        if list_exist {
            dentry_list.unwrap().extend(list);
        }
        Ok(dentry)
    }

    /// Resolves a symlink to its target dentry.
    ///
    /// This function reads the symlink file and finds the target dentry.
    ///
    /// `dentry` must be a valid symlink.
    ///
    /// Returns the target dentry if the target file exists.
    /// Returns an invalid dentry if the target file does not exist but its parent
    /// directory does.
    /// Returns an `ENOENT` error if any directory in the middle of the path does not
    /// exist.
    /// Returns an `ENOTDIR` error if any directory in the middle of the path is not
    /// a directory.
    /// Returns `ELOOP` error if it encounters too many symlinks.
    ///
    /// Note that the returned dentry may still be a symlink, and this function will
    /// not resolve it. You may not want to call this function on a symlink dentry
    /// generally; call [`Self::resolve_symlink_through`] instead.
    pub fn resolve_symlink(dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn Dentry>> {
        Self::resolve_symlink_recursive(dentry, &mut 1)
    }

    /// Do the same as [`Self::resolve_symlink`], but with a counter passed to
    /// [`Self::walk_recursive`] to help to limit the recursion depth.
    fn resolve_symlink_recursive(
        dentry: Arc<dyn Dentry>,
        counter: &mut usize,
    ) -> SysResult<Arc<dyn Dentry>> {
        debug_assert!(dentry.inode().unwrap().inotype() == InodeType::SymLink);

        const MAX_SYMLINK_DEPTH: usize = 40;
        *counter += 1;
        if *counter > MAX_SYMLINK_DEPTH {
            return Err(SysError::ELOOP);
        }

        let target_path = <dyn File>::open(Arc::clone(&dentry))?.readlink()?;
        Path::new(dentry.parent().unwrap(), target_path).walk_recursive(counter, None)
    }

    /// Do the same as [`Self::resolve_symlink`], but will resolve the symlink
    /// until it finds a non-symlink dentry.
    pub fn resolve_symlink_through(mut dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn Dentry>> {
        let mut counter = 0;
        loop {
            if dentry.is_negative() {
                return Err(SysError::ENOENT);
            }
            if dentry.inode().unwrap().inotype() != InodeType::SymLink {
                return Ok(dentry);
            }
            dentry = Self::resolve_symlink_recursive(Arc::clone(&dentry), &mut counter)?;
        }
    }
}

pub fn split_parent_and_name(path: &str) -> (String, Option<String>) {
    let mut trimmed_path = path.trim_start_matches('/').to_string();

    if let Some(n) = trimmed_path.rfind('/') {
        let mut m = n;
        if path.starts_with('/') {
            trimmed_path.insert(0, '/');
            m = m + 1;
        }
        (
            trimmed_path[..m].to_string(),
            Some(trimmed_path[m + 1..].to_string()),
        )
    } else {
        if path.starts_with('/') {
            trimmed_path.insert(0, '/');
        }
        (trimmed_path, None)
    }
}

pub fn test_split_parent_and_name() {
    struct PName {
        path: String,
        parent: String,
        child: Option<String>,
    }

    let create_test = |p: &str, pr: &str, ch: Option<&str>| PName {
        path: p.to_string(),
        parent: pr.to_string(),
        child: ch.map(|c| c.to_string()),
    };

    let tests = [
        create_test("/abs/ac", "/abs", Some("ac")), // 常规路径
        create_test("foo/bar/baz", "foo/bar", Some("baz")), // 多级相对路径
        create_test("/single", "/single", None),    // 只有一个路径元素
        create_test("/", "/", None),                // 只有根路径
        create_test("", "", None),                  // 空字符串
    ];

    for test in &tests {
        let (pr, ch) = split_parent_and_name(&test.path);

        assert_eq!(pr, test.parent, "Failed for path: {:?}", test.path);
        assert_eq!(ch, test.child, "Failed for path: {:?}", test.path);
    }

    log::info!("pass test_split_parent_and_name");
}
