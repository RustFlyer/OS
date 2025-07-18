//! Fanotify event file system implementation.
//!
//! This module provides the file system implementation for fanotify event files.
//! These are special files created when fanotify events are read from the fanotify group.
//! They appear as symbolic links in /proc/<pid>/fd/ pointing to the original files that
//! triggered the events.

use alloc::{string::String, sync::Arc};

use systype::error::SysResult;

use crate::{dentry::Dentry, fanotify::fs::superblock::SUPERBLOCK, file::File};

use super::super::FanotifyEntry;

use self::{dentry::FanotifyEventDentry, inode::FanotifyEventInode};

pub mod dentry;
pub mod file;
pub mod inode;

/// Creates an fanotify event file descriptor.
///
/// This function creates all the necessary VFS objects (inode, dentry, file) for
/// a fanotify event file and returns a [`File`] instance.
pub fn create_event_file(
    entry: Arc<FanotifyEntry>,
    target_path: String,
) -> SysResult<Arc<dyn File>> {
    let inode = FanotifyEventInode::new(SUPERBLOCK.clone(), entry, target_path);
    let dentry = FanotifyEventDentry::new(Some(inode), None);
    dentry.base_open()
}
