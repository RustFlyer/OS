use alloc::{string::String, sync::Arc};
use dentry::ZeroDentry;
use inode::ZeroInode;
use systype::error::SysResult;
use vfs::{dentry::Dentry, path::Path, sys_root_dentry};

pub mod dentry;
pub mod file;
pub mod inode;

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/zero");
    let path = Path::new(sys_root_dentry(), path);
    let zero_dentry = path.walk()?;
    let parent = zero_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = ZeroInode::new(parent.superblock().unwrap());
    let zero_dentry = ZeroDentry::new("zero", Some(inode), Some(weak_parent));
    parent.add_child(zero_dentry.clone());

    let sb = parent.clone().superblock();
    let zero_inode = ZeroInode::new(sb.clone().unwrap());
    zero_dentry.set_inode(zero_inode);
    log::debug!("success init zero");

    Ok(())
}
