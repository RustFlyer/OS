use alloc::boxed::Box;
use alloc::sync::Arc;
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::{
    event::{DetachedMount, MountAttr},
    flags::OpenTreeFlags,
    inode::OpenTreeInode,
};

pub struct OpenTreeFile {
    meta: FileMeta,
}

impl OpenTreeFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// Set the original path that was opened
    pub fn set_original_path(&self, path: alloc::string::String) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.set_original_path(path);
        Ok(())
    }

    /// Create a detached mount tree
    pub fn create_detached_mount(&self, source_path: &str, recursive: bool) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.create_detached_mount(source_path, recursive)
    }

    /// Get the detached mount tree
    pub fn get_detached_mount(&self) -> SysResult<Option<DetachedMount>> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(open_tree_inode.get_detached_mount())
    }

    /// Check if this represents a cloned mount
    pub fn is_cloned(&self) -> SysResult<bool> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(open_tree_inode.is_cloned())
    }

    /// Set target file descriptor for non-cloned case
    pub fn set_target_fd(&self, fd: u64) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.set_target_fd(fd);
        Ok(())
    }

    /// Get target file descriptor
    pub fn get_target_fd(&self) -> SysResult<Option<u64>> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(open_tree_inode.get_target_fd())
    }

    /// Update mount attributes
    pub fn set_mount_attributes(&self, attrs: MountAttr) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.set_mount_attributes(attrs)
    }

    /// Get mount attributes
    pub fn get_mount_attributes(&self) -> SysResult<MountAttr> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(open_tree_inode.get_mount_attributes())
    }

    /// Move the detached mount to a new location
    pub fn move_mount(&self, target_fd: i32, target_path: &str) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.move_mount(target_fd, target_path)
    }

    /// Get information about the mount tree
    pub fn get_mount_info(&self) -> SysResult<alloc::string::String> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.get_mount_info()
    }

    /// Get flags
    pub fn get_flags(&self) -> SysResult<OpenTreeFlags> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(open_tree_inode.get_flags())
    }

    /// Dissolve the detached mount
    pub fn dissolve_mount(&self) -> SysResult<()> {
        let inode = self.inode();
        let open_tree_inode = inode
            .downcast_arc::<OpenTreeInode>()
            .map_err(|_| SysError::EINVAL)?;

        open_tree_inode.dissolve_mount()
    }
}

#[async_trait]
impl File for OpenTreeFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        // open_tree file descriptors are O_PATH, so reading from them should return mount info
        let mount_info = self.get_mount_info()?;
        let info_bytes = mount_info.as_bytes();
        let copy_len = buf.len().min(info_bytes.len());

        buf[..copy_len].copy_from_slice(&info_bytes[..copy_len]);
        Ok(copy_len)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        // open_tree file descriptors are not writable
        Err(SysError::EBADF)
    }
}
