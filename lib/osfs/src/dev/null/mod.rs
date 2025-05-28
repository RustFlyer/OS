use alloc::{string::String, sync::Arc};
use dentry::NullDentry;
use inode::NullInode;
use systype::error::SysResult;
use vfs::{dentry::Dentry, path::Path, sys_root_dentry};

pub mod dentry;
pub mod file;
pub mod inode;

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/null");
    let path = Path::new(sys_root_dentry(), path);
    let null_dentry = path.walk()?;
    let parent = null_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = NullInode::new(parent.superblock().unwrap());
    let null_dentry = NullDentry::new("null", Some(inode), Some(weak_parent));
    parent.add_child(null_dentry.clone());

    let sb = parent.clone().superblock();
    let null_inode = NullInode::new(sb.clone().unwrap());
    null_dentry.set_inode(null_inode);

    Ok(())
}
