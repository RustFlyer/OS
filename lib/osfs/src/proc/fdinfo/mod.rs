pub mod dentry;
pub mod file;
pub mod info;
pub mod inode;

use alloc::sync::Arc;
use dentry::FdInfoDentry;
use inode::FdInfoInode;
use vfs::{dentry::Dentry, sys_root_dentry};

pub fn create_thread_fdinfo_file(tid: usize, fd: usize) {
    let root = sys_root_dentry();
    let proc_dentry = root.lookup("proc").unwrap();

    let num = alloc::format!("{}", tid);
    if proc_dentry.get_child(&num).is_none() {
        return;
    }

    let num_dentry = proc_dentry.get_child(&num).unwrap();
    let fdinfo_dentry = num_dentry.get_child("fdinfo").unwrap();

    // /proc/<tid>/fdinfo/<fd>
    let fd_inode = FdInfoInode::new(root.superblock().unwrap(), tid, fd);
    let fd_dentry: Arc<dyn Dentry> =
        FdInfoDentry::new(fd, Some(fd_inode), Some(Arc::downgrade(&fdinfo_dentry)));
    fdinfo_dentry.add_child(fd_dentry);
}
