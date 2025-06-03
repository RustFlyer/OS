use alloc::{string::String, sync::Arc};
use config::inode::InodeType;
use systype::error::SysResult;
use vfs::{dentry::Dentry, inode::Inode, path::Path, sys_root_dentry};

use crate::simple::{dentry::SimpleDentry, inode::SimpleInode};

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/shm");
    let path = Path::new(sys_root_dentry(), path);
    let shm_dentry = path.walk()?;

    let parent = shm_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = SimpleInode::new(parent.superblock().unwrap());
    let shm_dentry = SimpleDentry::new("shm", Some(inode.clone()), Some(weak_parent));
    inode.set_inotype(InodeType::Dir);

    parent.add_child(shm_dentry.clone());
    shm_dentry.set_inode(inode);
    log::debug!("success init shm");

    Ok(())
}
