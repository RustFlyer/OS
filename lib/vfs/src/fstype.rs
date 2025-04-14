use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};

use config::vfs::MountFlags;
use driver::BlockDevice;
use mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};

use crate::{dentry::Dentry, superblock::SuperBlock};

pub struct FileSystemTypeMeta {
    name: String,
    sblks: SpinNoIrqLock<BTreeMap<String, Arc<dyn SuperBlock>>>,
}

impl FileSystemTypeMeta {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sblks: SpinNoIrqLock::new(BTreeMap::new()),
        }
    }
}

pub trait FileSystemType: Send + Sync {
    fn get_meta(&self) -> &FileSystemTypeMeta;

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>>;

    fn kill_sblk(&self, sblk: Arc<dyn SuperBlock>) -> SysResult<()>;

    fn insert_sblk(&self, abs_path: &str, sblk: Arc<dyn SuperBlock>) {
        self.get_meta()
            .sblks
            .lock()
            .insert(abs_path.to_string(), sblk);
    }

    fn name(&self) -> String {
        self.get_meta().name.clone()
    }
}

impl dyn FileSystemType {
    /// file system mount
    ///
    /// Mounts the filesystem instance(`self`) under the `parent`-filesystem with `name`.
    ///
    /// # Arguments
    /// - `name`: Name of the mount point (e.g., "usr" for `/parent/usr`).
    /// - `parent`: Parent directory's dentry. If `None`, mounts as the root filesystem.
    /// - `flags`: Mount options (e.g., read-only, no-execute). See [`MountFlags`].
    /// - `dev`: Block device for storage-backed filesystems (e.g., `/dev/sda1`).
    ///   Virtual filesystems (e.g., devfs) should pass `None`.
    ///
    /// # Returns
    /// - `Ok(Arc<dyn Dentry>)`: Newly created dentry for the mount point.
    /// - `Err(SysError)`: If mounting fails (e.g., invalid device).
    pub fn mount(
        self: &Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        self.clone().base_mount(name, parent, flags, dev)
    }

    pub fn get_sb(&self, abs_path: &str) -> SysResult<Arc<dyn SuperBlock>> {
        self.get_meta()
            .sblks
            .lock()
            .get(abs_path)
            .cloned()
            .ok_or(SysError::ENOENT)
    }
}
