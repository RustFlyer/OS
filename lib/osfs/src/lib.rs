#![no_main]
#![no_std]
#![feature(new_zeroed_alloc)]

use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use config::vfs::MountFlags;
use dev::DevFsType;
use driver::{BLOCK_DEVICE, BlockDevice};
use mutex::SpinNoIrqLock;
use proc::{fs::ProcFsType, init_procfs};
use systype::{SysError, SysResult};
use tmp::TmpFsType;
use vfs::{SYS_ROOT_DENTRY, file::File, fstype::FileSystemType};

extern crate alloc;

pub mod dev;
pub mod fd_table;
pub mod pipe;
pub mod proc;
pub mod pselect;
pub mod simple;
pub mod tmp;

pub use vfs::sys_root_dentry;

pub static FS_MANAGER: SpinNoIrqLock<BTreeMap<String, Arc<dyn FileSystemType>>> =
    SpinNoIrqLock::new(BTreeMap::new());

type DiskFsType = ext4::fs::ExtFsType;
type DiskFsTypeFat = fat32::fs::FatFsType;

pub const DISK_FS_NAME: &str = "ext4";

pub fn get_block_device() -> SysResult<Arc<dyn BlockDevice>> {
    let devices = BLOCK_DEVICE.get().ok_or(SysError::ENODEV)?.clone();
    Ok(devices)
}

pub fn register_dev() {
    let devfs = DevFsType::new();
    FS_MANAGER.lock().insert(devfs.name(), devfs);

    let devfs2 = DiskFsTypeFat::new();
    FS_MANAGER.lock().insert(devfs2.name(), devfs2);

    let procfs = ProcFsType::new();
    FS_MANAGER.lock().insert(procfs.name(), procfs);

    let tmpfs = TmpFsType::new();
    FS_MANAGER.lock().insert(tmpfs.name(), tmpfs);
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

    let devfs = FS_MANAGER.lock().get("devfs").unwrap().clone();
    devfs
        .mount("dev", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    log::debug!("success mount devfs");

    let procfs = FS_MANAGER.lock().get("procfs").unwrap().clone();
    let procfs_dentry = procfs
        .mount("proc", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    init_procfs(procfs_dentry).unwrap();
    log::debug!("success mount procfs");

    let tmpfs = FS_MANAGER.lock().get("tmpfs").unwrap().clone();
    tmpfs
        .mount("tmp", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    log::debug!("success mount tmpfs");

    SYS_ROOT_DENTRY.call_once(|| diskfs_root);

    dev::tty::init().expect("dev-tty init fails");
    dev::rtc::init().expect("dev-rtc init fails");
    dev::null::init().expect("dev-null init fails");
    dev::shm::init().expect("dev-shm init fails");
    dev::zero::init().expect("dev-zero init fails");

    <dyn File>::open(sys_root_dentry())
        .unwrap()
        .load_dir()
        .unwrap();
}
