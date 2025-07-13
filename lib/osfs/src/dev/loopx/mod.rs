use alloc::{format, string::ToString, sync::Arc};
use dentry::LoopDentry;
use inode::LoopInode;
use systype::error::SysResult;
use vfs::{path::Path, sys_root_dentry};

pub mod dentry;
pub mod externf;
pub mod file;
pub mod inode;
pub mod loopinfo;

pub fn init() -> SysResult<()> {
    let path = Path::new(sys_root_dentry(), "/dev".to_string());
    let dev_dentry = path.walk()?;

    for i in 0..8 {
        let name = format!("loop{}", i);
        let inode = LoopInode::new(dev_dentry.superblock().unwrap(), i);
        let dentry = LoopDentry::new(&name, Some(inode), Some(Arc::downgrade(&dev_dentry)));
        dev_dentry.add_child(dentry);
    }

    Ok(())
}
