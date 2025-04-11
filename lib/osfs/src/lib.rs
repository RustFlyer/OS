#![no_main]
#![no_std]

use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use config::vfs::MountFlags;
use dev::DevFsType;
use driver::BLOCK_DEVICE;
use mutex::SpinNoIrqLock;
use vfs::{SYS_ROOT_DENTRY, file::File, fstype::FileSystemType};

extern crate alloc;

pub mod dev;
pub mod fd_table;
pub mod simple;
pub mod simplefile;

pub use vfs::sys_root_dentry;

pub static FS_MANAGER: SpinNoIrqLock<BTreeMap<String, Arc<dyn FileSystemType>>> =
    SpinNoIrqLock::new(BTreeMap::new());

type DiskFsType = ext4::fs::ExtFsType;

pub const DISK_FS_NAME: &str = "ext4";

pub fn register_dev() {
    let devfs = DevFsType::new();
    FS_MANAGER.lock().insert(devfs.name(), devfs);
}

pub fn init() {
    register_dev();

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

    // dev::tty::init().expect("dev-tty init fails");

    <dyn File>::open(sys_root_dentry())
        .unwrap()
        .load_dir()
        .unwrap();
}
