pub mod dentry;
pub mod file;
pub mod inode;
pub mod ioctl;
// pub mod queuebuffer;

use alloc::{string::String, sync::Arc};
use dentry::TtyDentry;
use file::TtyFile;
use inode::TtyInode;
use spin::Once;
use systype::error::SysResult;
use vfs::{file::File, path::Path};

pub use ioctl::TtyIoctlCmd;

use crate::sys_root_dentry;

pub static TTY0: Once<Arc<TtyFile>> = Once::new();
pub static TTY1: Once<Arc<TtyFile>> = Once::new();
pub static TTY2: Once<Arc<TtyFile>> = Once::new();

pub fn init() -> SysResult<()> {
    let file0 = create_tty_file(0)?;
    let file1 = create_tty_file(1)?;
    let file2 = create_tty_file(2)?;

    TTY0.call_once(|| file0);
    TTY1.call_once(|| file1);
    TTY2.call_once(|| file2);

    log::debug!("success init tty");
    Ok(())
}

pub fn create_tty_file(id: u64) -> SysResult<Arc<TtyFile>> {
    let path = Path::new(sys_root_dentry(), String::from("/dev/tty"));
    let tty_dentry = path.walk()?;
    let parent = tty_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);
    let inode = TtyInode::new(parent.superblock().unwrap(), id);
    let tty_dentry = TtyDentry::new("tty", Some(inode), Some(weak_parent));
    parent.add_child(tty_dentry.clone());
    Ok(TtyFile::new(tty_dentry))
}
