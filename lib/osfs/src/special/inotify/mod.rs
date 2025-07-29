use alloc::{string::ToString, sync::Arc};
use file::InotifyFile;
use flags::InotifyMask;
use mutex::SpinNoIrqLock;
use vfs::{file::File, inode::Inode};

pub mod dentry;
pub mod event;
pub mod file;
pub mod flags;
pub mod inode;

fn notify_inotify_event(inode_id: u64, mask: u32, name: Option<&str>) {
    static INOTIFY_REGISTRY: SpinNoIrqLock<alloc::vec::Vec<alloc::sync::Weak<InotifyFile>>> =
        SpinNoIrqLock::new(alloc::vec::Vec::new());

    let mut registry = INOTIFY_REGISTRY.lock();
    registry.retain(|weak_ref| {
        if let Some(inotify_file) = weak_ref.upgrade() {
            let _ = inotify_file.notify_event(inode_id, mask, name.map(|s| s.to_string()));
            true
        } else {
            false
        }
    });
}

// call when file created
pub fn vfs_create_notify(file: Arc<dyn File>) {
    let dentry = file.dentry();
    let filename = dentry.name();
    if dentry.parent().is_none() {
        return;
    }
    let parent_inode_id = dentry.parent().unwrap().inode().unwrap().get_meta().ino as u64;

    notify_inotify_event(
        parent_inode_id,
        InotifyMask::IN_CREATE.bits(),
        Some(filename),
    );
}

// call when file removed
pub fn vfs_delete_notify(file: Arc<dyn File>) {
    let dentry = file.dentry();
    let filename = dentry.name();
    if dentry.parent().is_none() {
        return;
    }
    let parent_inode_id = dentry.parent().unwrap().inode().unwrap().get_meta().ino as u64;

    notify_inotify_event(
        parent_inode_id,
        InotifyMask::IN_DELETE.bits(),
        Some(filename),
    );
}

// call when file modified
pub fn vfs_modify_notify(inode_id: u64) {
    notify_inotify_event(inode_id, InotifyMask::IN_MODIFY.bits(), None);
}
