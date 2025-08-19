use alloc::{string::String, sync::Arc};
use dentry::UrandomDentry;
use inode::UrandomInode;
use systype::error::SysResult;
use vfs::{path::Path, sys_root_dentry};

pub mod dentry;
pub mod file;
pub mod inode;

pub(crate) static mut URANDOM_SEED: usize = 0;

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/urandom");
    let path = Path::new(sys_root_dentry(), path);
    let urandom_dentry = path.walk()?;
    let parent = urandom_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = UrandomInode::new(parent.superblock().unwrap());
    let urandom_dentry = UrandomDentry::new("urandom", Some(inode), Some(weak_parent));
    parent.add_child(urandom_dentry.clone());

    log::debug!("success init urandom");

    Ok(())
}
