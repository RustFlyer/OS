#![no_main]
#![no_std]

use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use config::vfs::MountFlags;
use driver::BLOCK_DEVICE;
use mutex::SpinNoIrqLock;
use spin::Once;
use vfs::{dentry::Dentry, file::File, fstype::FileSystemType};

extern crate alloc;

pub mod fd_table;

pub static FS_MANAGER: SpinNoIrqLock<BTreeMap<String, Arc<dyn FileSystemType>>> =
    SpinNoIrqLock::new(BTreeMap::new());

static SYS_ROOT_DENTRY: Once<Arc<dyn Dentry>> = Once::new();

pub fn sys_root_dentry() -> Arc<dyn Dentry> {
    SYS_ROOT_DENTRY.get().unwrap().clone()
}

type DiskFsType = ext4::fs::ExtFsType;

pub const DISK_FS_NAME: &str = "ext4";

pub fn init() {
    let diskfs = DiskFsType::new();
    FS_MANAGER.lock().insert(diskfs.name(), diskfs);

    let diskfs = FS_MANAGER.lock().get(DISK_FS_NAME).unwrap().clone();
    log::debug!("get ext4 diskfs");

    let block_device = Some(BLOCK_DEVICE.get().unwrap().clone());
    log::debug!("get BLOCK_DEVICE");

    let diskfs_root = diskfs
        .mount("/", None, MountFlags::empty(), block_device)
        .unwrap();
    log::debug!("success mount diskfs");

    SYS_ROOT_DENTRY.call_once(|| diskfs_root);

    <dyn File>::open(sys_root_dentry()).unwrap().load_dir().unwrap();
}
