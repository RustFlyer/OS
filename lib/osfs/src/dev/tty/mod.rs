pub mod dentry;
pub mod file;
pub mod inode;
mod ioctl;
pub mod queuebuffer;

use alloc::{string::String, sync::Arc};
use dentry::TtyDentry;
use file::TtyFile;
use inode::TtyInode;
use spin::Once;
use systype::SysResult;
use vfs::{dentry::Dentry, path::Path};

use crate::sys_root_dentry;

pub static TTY: Once<Arc<TtyFile>> = Once::new();

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/tty");
    let path = Path::new(sys_root_dentry(), path);
    let tty_dentry = path.walk()?;
    let parent = tty_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = TtyInode::new(parent.superblock().unwrap());
    let tty_dentry = TtyDentry::new("tty", Some(inode), Some(weak_parent));
    parent.add_child(tty_dentry.clone());

    let sb = parent.clone().superblock();
    let tty_inode = TtyInode::new(sb.clone().unwrap());
    tty_dentry.set_inode(tty_inode);
    let tty_file = TtyFile::new(tty_dentry.clone());

    TTY.call_once(|| tty_file);
    Ok(())
}
