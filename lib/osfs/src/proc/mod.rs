use alloc::sync::Arc;
use config::inode::{InodeMode, InodeType};
use exe::{dentry::ExeDentry, inode::ExeInode};
use meminfo::{dentry::MemInfoDentry, inode::MemInfoInode};
use mounts::{dentry::MountsDentry, inode::MountsInode};
use systype::error::SysResult;
use vfs::{dentry::Dentry, inode::Inode};

use crate::{
    proc::status::{dentry::StatusDentry, inode::StatusInode},
    simple::{dentry::SimpleDentry, inode::SimpleInode},
};

pub mod exe;
pub mod meminfo;
pub mod mounts;
pub mod status;

pub mod fs;
pub mod superblock;

#[crate_interface::def_interface]
pub trait KernelProcIf {
    fn exe() -> alloc::string::String;
    fn status() -> alloc::string::String;
}

pub fn init_procfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    // /proc/meminfo
    let mem_info_inode = MemInfoInode::new(root_dentry.superblock().unwrap());
    let mem_info_dentry = MemInfoDentry::new(
        "meminfo",
        Some(mem_info_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(mem_info_dentry);

    // /proc/mounts
    let mounts_inode = MountsInode::new(root_dentry.superblock().unwrap());
    let mounts_dentry = MountsDentry::new(
        "mounts",
        Some(mounts_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(mounts_dentry);

    // /proc/sys
    let sys_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    sys_inode.set_inotype(InodeType::Dir);
    let sys_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("sys", Some(sys_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(sys_dentry.clone());
    log::info!("[init_procfs] add sys_dentry path = {}", sys_dentry.path());

    let kernel_dentry = SimpleDentry::new("kernel", None, Some(Arc::downgrade(&sys_dentry)));
    sys_dentry.mkdir(kernel_dentry.into_dyn_ref(), InodeMode::DIR)?;
    log::info!(
        "[init_procfs] add kernel_dentry path = {}",
        kernel_dentry.path()
    );

    // let pid_max_dentry = SimpleDentry::new(
    //     "pid_max",
    //     None,
    //     Some(Arc::downgrade(&kernel_dentry.clone().into_dyn())),
    // );
    // kernel_dentry
    //     .into_dyn()
    //     .create(pid_max_dentry.into_dyn_ref(), InodeMode::REG)?;
    // log::info!("[init_procfs] add pid_max_dentry");

    // let pid_max_file = pid_max_dentry.base_open()?;

    // block_on(async { pid_max_file.write("32768\0".as_bytes()).await })?;

    // /proc/cpuinfo
    let cpuinfo_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    cpuinfo_inode.set_inotype(InodeType::Dir);
    let cpuinfo_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "cpuinfo",
        Some(cpuinfo_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(cpuinfo_dentry.clone());

    // /proc/self
    let self_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    self_inode.set_inotype(InodeType::Dir);
    let self_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("self", Some(self_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(self_dentry.clone());

    // /proc/self/exe
    let exe_inode = ExeInode::new(root_dentry.superblock().unwrap());
    let exe_dentry: Arc<dyn Dentry> =
        ExeDentry::new(Some(exe_inode), Some(Arc::downgrade(&self_dentry)));
    log::info!("[init_procfs] add exe_dentry path = {}", exe_dentry.path());
    self_dentry.add_child(exe_dentry);

    // /proc/self/status
    let status_inode = StatusInode::new(root_dentry.superblock().unwrap());
    let status_dentry: Arc<dyn Dentry> =
        StatusDentry::new(Some(status_inode), Some(Arc::downgrade(&self_dentry)));
    log::info!(
        "[init_procfs] add status_dentry path = {}",
        status_dentry.path()
    );
    self_dentry.add_child(status_dentry);

    Ok(())
}
