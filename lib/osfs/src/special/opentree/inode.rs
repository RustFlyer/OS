use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::task::Waker;
use spin::Mutex;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::{
    event::{DetachedMount, MountAttr, MountTreeNode},
    flags::{MountFlags, OpenTreeFlags},
};

pub struct OpenTreeInode {
    meta: InodeMeta,
    flags: OpenTreeFlags,
    /// The detached mount tree (if OPEN_TREE_CLONE was used)
    detached_mount: Mutex<Option<DetachedMount>>,
    /// Original path that was opened
    original_path: Mutex<Option<alloc::string::String>>,
    /// Mount namespace ID this inode belongs to
    mount_ns_id: AtomicU64,
    /// Whether this represents a cloned mount
    is_cloned: AtomicBool,
    /// File descriptor that this tree references (for non-cloned case)
    target_fd: AtomicU64,
    /// Mount attributes
    mount_attrs: Mutex<MountAttr>,
    /// Wakers for async operations
    wakers: Mutex<alloc::vec::Vec<Waker>>,
}

impl OpenTreeInode {
    pub fn new(flags: OpenTreeFlags, mount_ns_id: u64) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            flags,
            detached_mount: Mutex::new(None),
            original_path: Mutex::new(None),
            mount_ns_id: AtomicU64::new(mount_ns_id),
            is_cloned: AtomicBool::new(false),
            target_fd: AtomicU64::new(u64::MAX),
            mount_attrs: Mutex::new(MountAttr::new()),
            wakers: Mutex::new(alloc::vec::Vec::new()),
        })
    }

    /// Set the original path that was opened
    pub fn set_original_path(&self, path: alloc::string::String) {
        *self.original_path.lock() = Some(path);
    }

    /// Get the original path
    pub fn get_original_path(&self) -> Option<alloc::string::String> {
        self.original_path.lock().clone()
    }

    /// Create a detached mount tree (for OPEN_TREE_CLONE)
    pub fn create_detached_mount(&self, source_path: &str, recursive: bool) -> SysResult<()> {
        // In a real implementation, this would:
        // 1. Look up the source mount point
        // 2. Create a copy of the mount tree
        // 3. Detach it from the namespace
        // 4. Store it in detached_mount

        // For now, create a simple mock mount
        let mount_node = MountTreeNode::new(
            alloc_ino() as u64,        // mount_id
            0,                         // parent_id (detached)
            8,                         // major (example)
            1,                         // minor (example)
            "/".to_string(),           // root
            source_path.to_string(),   // mount_point
            "rw,relatime".to_string(), // mount_options
            "ext4".to_string(),        // fs_type
            "/dev/sda1".to_string(),   // mount_source
            "rw,relatime".to_string(), // super_options
        );

        let detached = DetachedMount::new(mount_node, recursive);
        *self.detached_mount.lock() = Some(detached);
        self.is_cloned.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Get the detached mount tree
    pub fn get_detached_mount(&self) -> Option<DetachedMount> {
        self.detached_mount.lock().clone()
    }

    /// Check if this represents a cloned mount
    pub fn is_cloned(&self) -> bool {
        self.is_cloned.load(Ordering::Relaxed)
    }

    /// Set target file descriptor for non-cloned case
    pub fn set_target_fd(&self, fd: u64) {
        self.target_fd.store(fd, Ordering::Relaxed);
    }

    /// Get target file descriptor
    pub fn get_target_fd(&self) -> Option<u64> {
        let fd = self.target_fd.load(Ordering::Relaxed);
        if fd == u64::MAX { None } else { Some(fd) }
    }

    /// Update mount attributes
    pub fn set_mount_attributes(&self, attrs: MountAttr) -> SysResult<()> {
        if !self.is_cloned() {
            return Err(SysError::EINVAL);
        }

        *self.mount_attrs.lock() = attrs;
        Ok(())
    }

    /// Get mount attributes
    pub fn get_mount_attributes(&self) -> MountAttr {
        *self.mount_attrs.lock()
    }

    /// Get mount namespace ID
    pub fn get_mount_ns_id(&self) -> u64 {
        self.mount_ns_id.load(Ordering::Relaxed)
    }

    /// Get flags
    pub fn get_flags(&self) -> OpenTreeFlags {
        self.flags
    }

    /// Register a waker for async operations
    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.wakers.lock();
        if !wakers.iter().any(|w| w.will_wake(&waker)) {
            wakers.push(waker);
        }
    }

    /// Wake all registered wakers
    fn wake_all(&self) {
        let mut wakers = self.wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    /// Dissolve the detached mount (called on final close)
    pub fn dissolve_mount(&self) -> SysResult<()> {
        if !self.is_cloned() {
            return Ok(());
        }

        // In a real implementation, this would:
        // 1. Recursively unmount all mounts in the detached tree
        // 2. Release all resources
        // 3. Notify any waiters

        *self.detached_mount.lock() = None;
        self.wake_all();

        log::debug!(
            "[open_tree] Dissolved detached mount tree for inode {}",
            self.meta.ino
        );
        Ok(())
    }

    /// Move the detached mount to a new location
    pub fn move_mount(&self, target_fd: i32, target_path: &str) -> SysResult<()> {
        if !self.is_cloned() {
            return Err(SysError::EINVAL);
        }

        let detached = self.detached_mount.lock();
        if detached.is_none() {
            return Err(SysError::EINVAL);
        }

        // In a real implementation, this would:
        // 1. Validate the target location
        // 2. Attach the detached tree to the target
        // 3. Update the namespace
        // 4. Clear the detached mount from this inode

        log::debug!(
            "[open_tree] Moving detached mount to fd:{} path:{}",
            target_fd,
            target_path
        );

        Ok(())
    }

    /// Get information about the mount tree
    pub fn get_mount_info(&self) -> SysResult<alloc::string::String> {
        if let Some(ref detached) = *self.detached_mount.lock() {
            Ok(detached.serialize())
        } else if let Some(ref path) = *self.original_path.lock() {
            Ok(format!("open_tree reference to: {}", path))
        } else {
            Ok("empty open_tree".to_string())
        }
    }
}

impl Inode for OpenTreeInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}

// Implement Drop to automatically dissolve mount on final reference drop
impl Drop for OpenTreeInode {
    fn drop(&mut self) {
        let _ = self.dissolve_mount();
    }
}
