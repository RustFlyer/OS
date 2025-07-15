use alloc::{string::String, sync::Arc};
use dentry::FullDentry;
use inode::FullInode;
use systype::error::SysResult;
use vfs::{dentry::Dentry, path::Path, sys_root_dentry};

pub mod dentry;
pub mod file;
pub mod inode;

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/full");
    let path = Path::new(sys_root_dentry(), path);
    let full_dentry = path.walk()?;
    let parent = full_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = FullInode::new(parent.superblock().unwrap());
    let full_dentry = FullDentry::new("full", Some(inode), Some(weak_parent));
    parent.add_child(full_dentry.clone());

    let sb = parent.clone().superblock();
    let full_inode = FullInode::new(sb.clone().unwrap());
    full_dentry.set_inode(full_inode);
    log::debug!("success init full");

    Ok(())
}
