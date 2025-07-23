use alloc::{string::ToString, sync::Arc};
use dentry::FdDentry;
use inode::FdInode;
use systype::error::SysResult;
use vfs::{path::Path, sys_root_dentry};

pub mod dentry;
pub mod dirdentry;
pub mod file;
pub mod inode;

pub fn create_self_fd_file(fd: usize) -> SysResult<()> {
    let parent = Path::new(sys_root_dentry(), "/proc/self/fd".to_string())
        .walk()
        .expect("no fd dir");

    // let name = alloc::format!("{}", fd);
    // if parent.lookup(&name).is_ok() {
    //     return Ok(());
    // }

    let fdinode = FdInode::new(parent.superblock().unwrap(), fd);
    let fddentry = FdDentry::new(Some(fdinode), Some(Arc::downgrade(&parent)), fd);
    parent.add_child(fddentry);

    // log::error!("[create_self_fd_file] create fd{fd} file success");

    Ok(())
}
