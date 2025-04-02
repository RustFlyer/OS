extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use config::inode::InodeMode;
use mutex::SpinNoIrqLock;
use systype::{SysError, SysResult, SyscallResult};

use crate::file::File;
use crate::inode::Inode;
use crate::superblock::{self, SuperBlock};

pub struct DentryMeta {
    pub name: String,
    pub sblk: Weak<dyn SuperBlock>,
    pub pdentry: Option<Weak<dyn Dentry>>,

    pub inode: SpinNoIrqLock<Option<Arc<dyn Inode>>>,
    pub children: SpinNoIrqLock<BTreeMap<String, Arc<dyn Dentry>>>,
}

impl DentryMeta {
    pub fn new(
        name: &str,
        superblock: Arc<dyn SuperBlock>,
        parentdentry: Option<Arc<dyn Dentry>>,
    ) -> Self {
        let sblk = Arc::downgrade(&superblock);
        let inode = SpinNoIrqLock::new(None);
        let name = name.to_string();
        let children = SpinNoIrqLock::new(BTreeMap::new());
        let pdentry = if parentdentry.is_none() {
            None
        } else {
            Some(Arc::downgrade(&parentdentry.unwrap()))
        };

        Self {
            name,
            sblk,
            pdentry,
            inode,
            children,
        }
    }
}

pub trait Dentry: Send + Sync {
    fn get_meta(&self) -> &DentryMeta;

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>>;

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>>;

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>>;

    fn base_link(self: Arc<Self>, new: &Arc<dyn Dentry>) -> SysResult<()>;

    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult;

    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult;

    fn base_new_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry>;

    fn inode(&self) -> SysResult<Arc<dyn Inode>> {
        self.get_meta()
            .inode
            .lock()
            .as_ref()
            .ok_or(SysError::ENOENT)
            .cloned()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.get_meta().sblk.upgrade().unwrap()
    }

    fn name(&self) -> String {
        self.get_meta().name.clone()
    }

    fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.get_meta()
            .pdentry
            .as_ref()
            .map(|p| p.upgrade().unwrap())
    }

    fn children(&self) -> BTreeMap<String, Arc<dyn Dentry>> {
        self.get_meta().children.lock().clone()
    }

    fn get_child(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.get_meta().children.lock().get(name).cloned()
    }

    fn set_inode(&self, inode: Arc<dyn Inode>) {
        *self.get_meta().inode.lock() = Some(inode);
    }

    fn insert(&self, child: Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.get_meta().children.lock().insert(child.name(), child)
    }

    /// Get the path of this dentry.
    fn path(&self) -> String {
        let Some(parent) = self.parent() else {
            log::warn!("dentry has no parent");
            return String::from("/");
        };

        let mut current_segment = "/".to_string() + self.name().as_str();
        if current_segment == "//" {
            current_segment = String::new();
        }

        if parent.name() == "/" {
            match parent.parent() {
                Some(grandparent) => grandparent.path() + &current_segment,
                None => current_segment,
            }
        } else {
            parent.path() + &current_segment
        }
    }
}

impl dyn Dentry {
    pub fn is_negetive(&self) -> bool {
        self.get_meta().inode.lock().is_none()
    }

    pub fn clear_inode(&self) {
        *self.get_meta().inode.lock() = None;
    }

    pub fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.get_meta().children.lock().remove(name)
    }

    pub fn open(self: &Arc<Self>) -> SysResult<Arc<dyn File>> {
        self.clone().base_open()
    }

    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let child = self.get_child(name);
        if child.is_some() {
            log::trace!(
                "[Dentry::lookup] lookup {name} in cache in path {}",
                self.path()
            );
            return Ok(child.unwrap());
        }
        log::trace!(
            "[Dentry::lookup] lookup {name} not in cache in path {}",
            self.path()
        );
        self.clone().base_lookup(name)
    }

    pub fn create(self: &Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        self.clone().base_create(name, mode)
    }

    pub fn unlink(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_unlink(name)
    }

    pub fn rmdir(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_rmdir(name)
    }

    pub fn new_child(self: &Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let child = self.clone().base_new_child(name);
        child
    }

    pub fn get_child_or_create(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        self.get_child(name).unwrap_or_else(|| {
            let new_dentry = self.clone().new_child(name);
            self.insert(new_dentry.clone());
            new_dentry
        })
    }
}
