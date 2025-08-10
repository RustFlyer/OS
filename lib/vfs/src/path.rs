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
    /// from the `start` dentry. `path` can be an empty string, which means the `start`
    /// dentry itself is the target.
    ///
    /// See [`Self::walk`] for more details on how the path is resolved and what errors
    /// may be returned. Note that `start` may be a negative dentry or not a directory,
    /// in which case corresponding errors will be returned when calling [`Self::walk`].
    ///
    /// This function does not check the validity of the path. If there are illegal
    /// characters in `path`, or if `path` is empty, the behavior is undefined. The
    /// caller must ensure that `path` is a valid path string.
    pub fn new(start: Arc<dyn Dentry>, path: String) -> Self {
        debug_assert!(!path.contains('\0'));
        Self { start, path }
    }

    /// Walks the path to find the target dentry.
    ///
    /// Returns a valid dentry if the target file exists.
    /// Returns an invalid (negative) dentry if the target file does not exist but its
    /// parent directory does.
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

/// Splits a path into its parent directory and the name of the file or directory.
///
/// `path` must not be an empty string.
///
/// Returns a tuple where the second element is the name of the file or directory, and
/// the first element is path to the parent directory. If the path consists of only a
/// single file name (e.g., "/", "foo", "bar/", etc.), the parent directory is an empty
/// string. The returned parent path is absolute if the input path is absolute, and
/// relative if the input path is relative.
///
/// Whether the returned parent path has trailing slashes, and whether it has consecutive
/// slashes, is undefined. Since such paths are valid paths, they should be acceptable.
///
/// # Example
/// ```rust
/// // `baz` may be a file or directory.
/// let (parent, name) = split_parent_and_name("/foo/bar/baz");
/// assert_eq!(parent, "/foo/bar".to_string());
/// assert_eq!(name, "baz".to_string());
///
/// // The path may be a relative path.
/// let (parent, name) = split_parent_and_name("foo/bar/baz");
/// assert_eq!(parent, "foo/bar".to_string());
/// assert_eq!(name, "baz".to_string());
///
/// // The root directory may be a parent.
/// let (parent, name) = split_parent_and_name("/foo");
/// assert_eq!(parent, "/".to_string());
/// assert_eq!(name, "foo".to_string());
///
/// // If the path is just a root directory, the parent is `None` and
/// // the name is "/".
/// let (parent, name) = split_parent_and_name("/");
/// assert_eq!(parent, "".to_string());
/// assert_eq!(name, "".to_string());
///
/// // If the path is just a file name, the parent is `None` and the
/// // name is the whole path.
/// let (parent, name) = split_parent_and_name("foo");
/// assert_eq!(parent, "".to_string());
/// assert_eq!(name, "foo".to_string());
/// ```
pub fn split_parent_and_name(path: &str) -> (String, String) {
    debug_assert!(!path.is_empty());

    // Remove trailing slashes
    let path = path.trim_end_matches('/');

    if path.is_empty() {
        // The root directory.
        return ("".to_string(), String::new());
    }

    if let Some(last_slash_index) = path.rfind('/') {
        let parent = &path[..last_slash_index];
        let name = &path[last_slash_index + 1..];
        if parent.is_empty() {
            ("/".to_string(), name.to_string())
        } else {
            (parent.to_string(), name.to_string())
        }
    } else {
        // No slashes found, so the whole path is the name
        ("".to_string(), path.to_string())
    }
}

pub fn test_split_parent_and_name() {
    struct PName {
        path: String,
        parent: String,
        child: String,
    }

    fn create_test(path: &str, parent: &str, child: &str) -> PName {
        PName {
            path: path.to_string(),
            parent: parent.to_string(),
            child: child.to_string(),
        }
    }

    let tests = [
        create_test("/foo/bar/baz", "/foo/bar", "baz"),
        create_test("/foo/bar/", "/foo", "bar"),
        create_test("/foo", "/", "foo"),
        create_test("/", "", ""),
        create_test("foo", "", "foo"),
        create_test("foo/bar/baz", "foo/bar", "baz"),
        create_test("foo/bar/", "foo", "bar"),
        create_test("foo", "", "foo"),
    ];

    for test in &tests {
        let (parent, child) = split_parent_and_name(&test.path);

        assert_eq!(parent, test.parent, "Failed for path: {:?}", test.path);
        assert_eq!(child, test.child, "Failed for path: {:?}", test.path);
    }

    log::info!("pass test_split_parent_and_name");
}
