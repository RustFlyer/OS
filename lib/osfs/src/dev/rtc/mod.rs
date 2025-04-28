use alloc::{string::String, sync::Arc};
use dentry::RtcDentry;
use file::RtcFile;
use inode::RtcInode;
use systype::SysResult;
use vfs::{dentry::Dentry, path::Path, sys_root_dentry};

pub mod dentry;
pub mod file;
pub mod inode;
pub mod ioctl;

#[derive(Default)]
#[repr(C)]
pub struct RtcTime {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
}

pub fn init() -> SysResult<()> {
    let path = String::from("/dev/rtc");
    let path = Path::new(sys_root_dentry(), path);
    let rtc_dentry = path.walk()?;
    let parent = rtc_dentry.parent().unwrap();
    let weak_parent = Arc::downgrade(&parent);

    let inode = RtcInode::new(parent.superblock().unwrap());
    let rtc_dentry = RtcDentry::new("rtc", Some(inode), Some(weak_parent));
    parent.add_child(rtc_dentry.clone());

    let sb = parent.clone().superblock();
    let rtc_inode = RtcInode::new(sb.clone().unwrap());
    rtc_dentry.set_inode(rtc_inode);
    // let rtc_file = RtcFile::new(rtc_dentry.clone());

    Ok(())
}
