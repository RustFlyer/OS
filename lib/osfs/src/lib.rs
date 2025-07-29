#![no_main]
#![no_std]
#![feature(new_zeroed_alloc)]
#![feature(sync_unsafe_cell)]

use alloc::{
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};
use config::vfs::MountFlags;
use dev::DevFsType;
use driver::{BLOCK_DEVICE, BlockDevice, println};
use mutex::SpinNoIrqLock;
use proc::{fs::ProcFsType, init_procfs};
use systype::error::{SysError, SysResult};
use tmp::TmpFsType;
use vfs::{SYS_ROOT_DENTRY, file::File, fstype::FileSystemType};

use etc::*;

extern crate alloc;

pub mod dev;
pub mod etc;
pub mod fd_table;
pub mod passwd;
pub mod pipe;
pub mod proc;
pub mod pselect;
pub mod simple;
pub mod special;
pub mod sys;
pub mod tmp;
pub mod var;

pub use vfs::sys_root_dentry;

use crate::{
    etc::fs::EtcFsType,
    sys::{fs::SysFsType, init_sysfs},
    var::VarFsType,
};

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
    let diskfs = DiskFsType::new();
    FS_MANAGER.lock().insert(diskfs.name(), diskfs);

    let diskfsusr = DiskFsType::new();
    FS_MANAGER.lock().insert("usr".to_string(), diskfsusr);

    let devfs = DevFsType::new();
    FS_MANAGER.lock().insert(devfs.name(), devfs);

    let fatfs = DiskFsTypeFat::new();
    FS_MANAGER.lock().insert(fatfs.name(), fatfs);

    let procfs = ProcFsType::new();
    FS_MANAGER.lock().insert(procfs.name(), procfs);

    let tmpfs = TmpFsType::new();
    FS_MANAGER.lock().insert(tmpfs.name(), tmpfs);

    let varfs = VarFsType::new();
    FS_MANAGER.lock().insert(varfs.name(), varfs);

    let sysfs = SysFsType::new();
    FS_MANAGER.lock().insert(sysfs.name(), sysfs);

    let etcfs = EtcFsType::new();
    FS_MANAGER.lock().insert(etcfs.name(), etcfs);
}

pub fn init() {
    register_dev();

    let diskfs = FS_MANAGER.lock().get(DISK_FS_NAME).unwrap().clone();
    println!("Get Ext4 Diskfs");

    let block_device = Some(BLOCK_DEVICE.get().unwrap().clone());
    println!("Get BLOCK_DEVICE");

    let diskfs_root = diskfs
        .mount("/", None, MountFlags::empty(), block_device)
        .unwrap();
    println!("success mount diskfs");

    // let block_device2 = Some(BLOCK_DEVICE.get().unwrap().clone());
    // log::debug!("get BLOCK_DEVICE2");

    // let usrfs = FS_MANAGER.lock().get("usr").unwrap().clone();
    // usrfs
    //     .mount(
    //         "usr",
    //         Some(diskfs_root.clone()),
    //         MountFlags::empty(),
    //         block_device2,
    //     )
    //     .unwrap();
    // log::debug!("success mount usrfs");

    let devfs = FS_MANAGER.lock().get("devfs").unwrap().clone();
    devfs
        .mount("dev", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    println!("success mount devfs");

    let procfs = FS_MANAGER.lock().get("procfs").unwrap().clone();
    let procfs_dentry = procfs
        .mount("proc", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    init_procfs(procfs_dentry).unwrap();
    println!("success mount procfs");

    let tmpfs = FS_MANAGER.lock().get("tmpfs").unwrap().clone();
    tmpfs
        .mount("tmp", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    println!("success mount tmpfs");

    // let varfs = FS_MANAGER.lock().get("varfs").unwrap().clone();
    // varfs
    //     .mount("var", Some(diskfs_root.clone()), MountFlags::empty(), None)
    //     .unwrap();
    // log::debug!("success mount varfs");

    let sysfs = FS_MANAGER.lock().get("sysfs").unwrap().clone();
    let sysfs_dentry = sysfs
        .mount("sys", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    init_sysfs(sysfs_dentry).unwrap();
    println!("success mount sysfs");

    let etcfs = FS_MANAGER.lock().get("etcfs").unwrap().clone();
    let etcfs_dentry = etcfs
        .mount("etc", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    init_etcfs(etcfs_dentry).unwrap();
    println!("success mount etcfs");

    SYS_ROOT_DENTRY.call_once(|| diskfs_root);
    println!("success init disk root");

    dev::tty::init().expect("dev-tty init fails");
    dev::rtc::init().expect("dev-rtc init fails");
    dev::null::init().expect("dev-null init fails");
    dev::shm::init().expect("dev-shm init fails");
    dev::zero::init().expect("dev-zero init fails");
    dev::urandom::init().expect("dev-urandom init fails");
    dev::loopx::init().expect("dev-loopx init fails");
    dev::full::init().expect("dev-full init fails");

    <dyn File>::open(sys_root_dentry())
        .unwrap()
        .load_dir()
        .unwrap();

    passwd::login_user();
}
