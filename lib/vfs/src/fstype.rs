use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};

use config::vfs::MountFlags;
use driver::BlockDevice;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};

use crate::{dentry::Dentry, inode::Inode, superblock::SuperBlock};

pub struct FileSystemTypeMeta {
    name: String,
    pub sblks: SpinNoIrqLock<BTreeMap<String, Arc<dyn SuperBlock>>>,
    pub inodes: SpinNoIrqLock<Vec<Weak<dyn Inode>>>,
}

impl FileSystemTypeMeta {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sblks: SpinNoIrqLock::new(BTreeMap::new()),
            inodes: SpinNoIrqLock::new(Vec::new()),
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

    /// this function can look for inode in queue in each filesystem and
    /// return the inode which has inodeid.
    ///
    /// when a dentry is opened, the related inode is pushed into the queue.
    fn get_inode_by_id(&self, inodeid: u64) -> Option<Arc<dyn Inode>> {
        let lock = &self.get_meta().inodes;
        let mut inodes = lock.lock();

        let mut rm_vec = Vec::new();
        let mut ret_inode = None;

        // find specified inode
        for (id, inode) in inodes.iter().enumerate() {
            let inode = inode.upgrade();
            if inode.is_some() {
                let inode = inode.unwrap();
                if inode.ino() == inodeid as i32 {
                    ret_inode = Some(inode);
                    break;
                }
                continue;
            }

            rm_vec.push(id);
        }

        // remove dead inode
        rm_vec.reverse();
        for id in rm_vec {
            inodes.remove(id);
        }

        ret_inode
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
