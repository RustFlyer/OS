use alloc::sync::Arc;

use systype::error::SysResult;

use crate::{dentry::Dentry, file::File, superblock::SuperBlock};

use super::{
    FanotifyGroup,
    fs::{dentry::FanotifyGroupDentry, inode::FanotifyGroupInode},
};

pub mod dentry;
pub mod file;
pub mod filesystem;
pub mod inode;
pub mod superblock;

/// Creates a dentry and file for the fanotify group.
///
/// This method creates the necessary VFS objects to represent the fanotify group
/// as a file descriptor that can be used to read events and write responses.
pub fn create_group_file(
    group: &Arc<FanotifyGroup>,
    superblock: Arc<dyn SuperBlock>,
) -> SysResult<Arc<dyn File>> {
    let inode = FanotifyGroupInode::new(superblock, group.clone());
    let dentry = FanotifyGroupDentry::new(Some(inode));
    dentry.base_open()
}
