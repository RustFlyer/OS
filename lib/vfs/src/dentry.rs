use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use config::inode::InodeMode;
use mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};

use crate::file::File;
use crate::inode::Inode;
use crate::superblock::SuperBlock;

/// Data that is common to all dentries.
pub struct DentryMeta {
    /// Name of the dentry.
    pub name: String,
    /// Parent dentry. This field is `None` if this dentry is the root of the filesystem.
    pub parent: Option<Weak<dyn Dentry>>,
    /// Children dentries of this dentry.
    pub children: SpinNoIrqLock<BTreeMap<String, Arc<dyn Dentry>>>,
    /// Inode that this dentry points to. This field is `None` if this dentry is a negative
    /// dentry.
    pub inode: SpinNoIrqLock<Option<Arc<dyn Inode>>>,
}

impl DentryMeta {
    /// Creates a new dentry metadata, with the given name, inode, and parent dentry.
    /// The newly created dentry has no children.
    pub fn new(
        name: &str,
        inode: Option<Arc<dyn Inode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            parent,
            children: SpinNoIrqLock::new(BTreeMap::new()),
            inode: SpinNoIrqLock::new(inode),
        }
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum DentryState {
    #[default]
    UnInit,
    Sync,
    Dirty,
}

pub trait Dentry: Send + Sync {
    /// Returns the metadata of this dentry.
    fn get_meta(&self) -> &DentryMeta;

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>>;

    /// Creates a file in directory `self` with the name given in `dentry` and the mode
    /// given in `mode`.
    ///
    /// `self` must be a valid directory. `dentry` must be a negative dentry and a child of `self`.
    /// After this call, `dentry` will become valid.
    fn base_create(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()>;

    /// Looks up on the disk for the dentry with the name given in `dentry` in directory `self`.
    ///
    /// `self` must be a valid directory. `dentry` must be a negative dentry and a child of `self`.
    /// After this call, `dentry` will become valid if the dentry exists (and the function returns
    /// `Ok(())`), or invalid if the dentry does not exist (and the function returns `Err(ENOENT)`).
    ///
    /// # Errors
    /// Returns `ENOENT` if the dentry does not exist. Other errors may be returned if the
    /// filesystem encounters any error while looking up the dentry.
    fn base_lookup(&self, dentry: &dyn Dentry) -> SysResult<()>;

    /// Creates a hard link in directory `self` with the name given in `dentry` to the file
    /// `inode`.
    ///
    /// `self` must be a valid directory and `dentry` must be a negative dentry
    /// and a child of `self`. After this call, `dentry` will become valid.
    /// The file type of `inode` must not be a directory, and `inode` and `dentry`
    /// must be in the same filesystem.
    fn base_link(&self, dentry: &dyn Dentry, old_dentry: &dyn Dentry) -> SysResult<()>;

    /// Removes the child dentry from directory `self`.
    ///
    /// `self` must be a valid directory. `dentry` must be a valid dentry and a child of
    /// `self`. After this call, `dentry` will become invalid. `dentry` must not be a directory.
    fn base_unlink(&self, dentry: &dyn Dentry) -> SysResult<()>;

    /// Removes the child directory `dentry` from directory `self`.
    ///
    /// `self` must be a valid directory. `dentry` must be a valid dentry and a child of
    /// `self`. After this call, `dentry` will become invalid. `dentry` must be a empty directory.
    fn base_rmdir(&self, dentry: &dyn Dentry) -> SysResult<()>;

    /// Constructs a new negative child dentry with the given name in directory `self`.
    ///
    /// `self` must be a valid directory.
    fn base_new_neg_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry>;

    /// Returns the inode of this dentry.
    fn inode(&self) -> Option<Arc<dyn Inode>> {
        self.get_meta().inode.lock().clone()
    }

    /// Sets the inode of this dentry.
    fn set_inode(&self, inode: Arc<dyn Inode>) {
        *self.get_meta().inode.lock() = Some(inode);
    }

    /// Returns whether this dentry is a negative dentry.
    fn is_negative(&self) -> bool {
        self.get_meta().inode.lock().is_none()
    }

    /// Returns the superblock pointed at by the inode of this dentry.
    ///
    /// Returns `None` if this dentry is a negative dentry, in which case getting the
    /// superblock seems to be meaningless.
    fn superblock(&self) -> Option<Arc<dyn SuperBlock>> {
        Some(self.inode()?.get_meta().superblock.clone())
    }

    /// Returns the name of this dentry.
    fn name(&self) -> &str {
        &self.get_meta().name
    }

    /// Returns a reference to the parent dentry of this dentry.
    ///
    /// Returns `None` if this dentry is the root.
    fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.get_meta()
            .parent
            .clone()
            .map(|parent| parent.upgrade().unwrap())
    }

    /// Returns a reference to the child dentry with the given name.
    ///
    /// Returns `None` if the child dentry does not exist, or is not constructed in
    /// the dentry tree.
    fn get_child(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.get_meta().children.lock().get(name).cloned()
    }

    /// Adds a child dentry to this dentry.
    fn add_child(&self, child: Arc<dyn Dentry>) {
        self.get_meta()
            .children
            .lock()
            .insert(child.name().to_string(), child);
    }

    /// Returns the path of this dentry as a string.
    ///
    /// The path is in the format of `/path/to/dentry`, always with no trailing `/`.
    /// However, the path of the root dentry is `/`.
    fn path(&self) -> String {
        let Some(parent) = self.parent() else {
            return String::from("/");
        };

        let parent_path = parent.path();
        if parent_path == "/" {
            parent_path + self.name()
        } else {
            parent_path + "/" + self.name()
        }
    }
}

impl dyn Dentry {
    /// Creates a `File` object pointing to dentry `self` and returns it.
    ///
    /// Returns an `ENOENT` error if this dentry is a negative dentry.
    pub fn open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        if self.is_negative() {
            return Err(SysError::ENOENT);
        }
        Arc::clone(&self).base_open()
    }

    /// Creates a regular file in directory `self` with the name given in `dentry` and the mode
    /// given in `mode`.
    ///
    /// `self` must be a valid directory. `dentry` must be a negative dentry and a child of `self`.
    /// After this call, `dentry` will become valid. The file type of `mode` must be a regular
    /// file.
    pub fn create(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        debug_assert!(dentry.is_negative());
        debug_assert!(mode.to_type().is_reg());
        self.base_create(dentry, mode)
    }

    /// Creates a directory in directory `self` with the name given in `dentry` and the mode
    /// given in `mode`.
    ///
    /// `self` must be a valid directory. `dentry` must be a negative dentry and a child of `self`.
    /// After this call, `dentry` will become valid. The file type of `mode` must be a directory.
    pub fn mkdir(&self, dentry: &dyn Dentry, mode: InodeMode) -> SysResult<()> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        debug_assert!(dentry.is_negative());
        debug_assert!(mode.to_type().is_dir());
        self.base_create(dentry, mode)
    }

    pub fn symlink(&self, _dentry: &dyn Dentry, _target: &str) -> SysResult<()> {
        unimplemented!("`symlink` is not implemented yet")
    }

    pub fn mknod(&self, _dentry: &dyn Dentry, _mode: InodeMode, _device: usize) -> SysResult<()> {
        unimplemented!("`mknod` seems not required in test cases")
    }

    /// Returns a reference to directory `self`'s child dentry which has the given name.
    /// The returned dentry may be a negative dentry.
    ///
    /// `self` must be a valid directory.
    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        match self.get_child(name) {
            Some(dentry) => Ok(dentry),
            None => {
                let dentry = self.new_neg_child(name);
                match self.base_lookup(dentry.as_ref()) {
                    Ok(_) | Err(SysError::ENOENT) => Ok(dentry),
                    Err(e) => {
                        log::warn!("Failed to lookup dentry: {:?}", e);
                        Err(e)
                    }
                }
            }
        }
    }

    /// Creates a hard link in directory `self` with the name given in `new_dentry` to the file
    /// `old_dentry`.
    ///
    /// `self` must be a valid directory. `old_dentry` must be a valid dentry. `new_dentry` must
    /// be a negative dentry and a child of `self`. After this call, `new_dentry` will become
    /// valid. `old_dentry` and `new_dentry` must be in the same filesystem. The file type of
    /// `old_dentry` must not be a directory.
    pub fn link(&self, old_dentry: &dyn Dentry, new_dentry: &dyn Dentry) -> SysResult<()> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        debug_assert!(!old_dentry.is_negative());
        debug_assert!(new_dentry.is_negative());
        debug_assert!(Arc::ptr_eq(
            &old_dentry.inode().unwrap().get_meta().superblock,
            &new_dentry.inode().unwrap().get_meta().superblock
        ));
        self.base_link(new_dentry, old_dentry)
    }

    /// Removes the child dentry from directory `self`.
    ///
    /// `self` must be a valid directory. `dentry` must be a valid dentry and a child of
    /// `self`. After this call, `dentry` will become invalid. `dentry` must not be a directory.
    pub fn unlink(&self, dentry: &dyn Dentry) -> SysResult<()> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        debug_assert!(!dentry.is_negative());
        debug_assert!(!dentry.inode().unwrap().inotype().is_dir());
        self.base_unlink(dentry)
    }

    /// Removes the child directory `dentry` from directory `self`.
    ///
    /// `self` must be a valid directory. `dentry` must be a valid dentry and a child of
    /// `self`. After this call, `dentry` will become invalid. `dentry` must be a directory.
    pub fn rmdir(&self, dentry: &dyn Dentry) -> SysResult<()> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        debug_assert!(!dentry.is_negative());
        debug_assert!(dentry.inode().unwrap().inotype().is_dir());
        self.base_rmdir(dentry)
    }

    /// Creates a new negative child dentry with the given name in directory `self`.
    ///
    /// This dentry must be a valid directory.
    pub fn new_neg_child(self: &Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        debug_assert!(!self.is_negative());
        debug_assert!(self.inode().unwrap().inotype().is_dir());
        Arc::clone(self).base_new_neg_child(name)
    }
}
